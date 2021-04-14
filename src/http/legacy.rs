use reqwest::blocking::{Client, ClientBuilder};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::redirect::Policy;

use crate::settings::global_user::GlobalUser;
use crate::{
    http::{feature::headers, Feature},
    settings::http_config::HttpConfig,
};

// TODO: remove this and replace it entirely with cloudflare-rs
pub fn client(http_config: Option<&HttpConfig>) -> Client {
    builder(http_config.unwrap_or(&Default::default()))
        .default_headers(headers(None))
        .build()
        .expect("could not create http client")
}

pub fn legacy_auth_client(user: &GlobalUser) -> Client {
    get_client(user, None)
}

pub fn featured_legacy_auth_client(user: &GlobalUser, feature: Feature) -> Client {
    get_client(user, Some(feature))
}

fn get_client(user: &GlobalUser, feature: Option<Feature>) -> Client {
    let mut headers = headers(feature);
    add_auth_headers(&mut headers, user);

    builder(user.get_http_config())
        .default_headers(headers)
        .redirect(Policy::none())
        .build()
        .expect("could not create authenticated http client")
}

fn builder(http_config: &HttpConfig) -> ClientBuilder {
    let builder = reqwest::blocking::Client::builder();
    builder
        .connect_timeout(http_config.get_connect_timeout())
        .timeout(http_config.get_http_timeout())
}

fn add_auth_headers(headers: &mut HeaderMap, user: &GlobalUser) {
    match user {
        GlobalUser::TokenAuth { api_token, .. } => {
            headers.insert(
                "Authorization",
                HeaderValue::from_str(&format!("Bearer {}", &api_token)).unwrap(),
            );
        }
        GlobalUser::GlobalKeyAuth { email, api_key, .. } => {
            headers.insert("X-Auth-Email", HeaderValue::from_str(&email).unwrap());
            headers.insert("X-Auth-Key", HeaderValue::from_str(&api_key).unwrap());
        }
    }
}
