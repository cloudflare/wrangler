use eventual::Timer;
use indicatif::{ProgressBar, ProgressStyle};
use openssl::base64;
use openssl::rsa::{Padding, Rsa};
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use serde::Deserialize;
use std::collections::HashMap;
use std::str;

use crate::commands::config::global_config;
use crate::settings::global_user::GlobalUser;
use crate::terminal::{interactive, open_browser};

pub fn run() -> Result<(), failure::Error> {
    let rsa = Rsa::generate(1024)?;
    let pubkey = rsa.public_key_to_pem_pkcs1()?;

    // Convert key to string and remove header and footer
    let pubkey_str = str::from_utf8(&pubkey)?;
    let pubkey_filtered = pubkey_str
        .lines()
        .filter(|line| !line.starts_with("---"))
        .fold(String::new(), |mut data, line| {
            data.push_str(&line);
            data
        });
    let pubkey_encoded = percent_encode(pubkey_filtered.as_bytes(), NON_ALPHANUMERIC).to_string();

    let browser_permission =
        interactive::confirm("Allow Wrangler to open a page in your browser?")?;
    if !browser_permission {
        failure::bail!("In order to log in you must allow Wrangler to open your browser. If you don't want to do this consider using `wrangler config`");
    }

    open_browser(&format!(
        "https://dash.cloudflare.com/wrangler?key={0}",
        pubkey_encoded
    ))?;

    let encrypted_token_str = poll_token(pubkey_filtered)?;
    let encrypted_token = base64::decode_block(encrypted_token_str.as_str())?;
    let mut token_bytes: [u8; 128] = [0; 128];

    rsa.private_decrypt(&encrypted_token, &mut token_bytes, Padding::PKCS1)?;
    let token = str::from_utf8(&token_bytes)?.trim_matches(char::from(0));

    let user = GlobalUser::TokenAuth {
        api_token: token.to_string(),
        http_config: Default::default(),
    };
    global_config(&user, true)?;

    Ok(())
}

#[derive(Deserialize)]
struct TokenResponse {
    result: String,
}

/// Poll for token, bail after 500 seconds.
fn poll_token(token_id: String) -> Result<String, failure::Error> {
    let mut request_params = HashMap::new();
    request_params.insert("token-id", token_id);

    let client = reqwest::blocking::Client::builder().build()?;
    let timer = Timer::new().interval_ms(1000).iter();

    let style = ProgressStyle::default_spinner().template("{spinner}   {msg}");
    let spinner = ProgressBar::new_spinner().with_style(style);
    spinner.set_message("Waiting for API token...");
    spinner.enable_steady_tick(20);

    for (seconds, _) in timer.enumerate() {
        let res = client
            .get("https://api.cloudflare.com/client/v4/workers/token")
            .json(&request_params)
            .send()?;

        if res.status().is_success() {
            let body: TokenResponse = res.json()?;
            return Ok(body.result);
        }

        if seconds >= 500 {
            break;
        }
    }

    failure::bail!(
        "Timed out while waiting for API token. Try using `wrangler config` if login fails to work."
    );
}

#[cfg(test)]
mod tests {
    use openssl::rsa::Rsa;

    #[test]
    fn test_rsa() {
        let rsa = Rsa::generate(1024).unwrap();
        rsa.public_key_to_pem_pkcs1().unwrap();
    }
}
