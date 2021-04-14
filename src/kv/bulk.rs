use indicatif::ProgressBar;

use cloudflare::endpoints::workerskv::delete_bulk::DeleteBulk;
use cloudflare::endpoints::workerskv::write_bulk::KeyValuePair;
use cloudflare::endpoints::workerskv::write_bulk::WriteBulk;
use cloudflare::framework::apiclient::ApiClient;
use cloudflare::framework::auth::Credentials;
use cloudflare::framework::{Environment, HttpApiClient, HttpApiClientConfig};

use crate::commands::kv::format_error;
use crate::http::feature::headers;
use crate::settings::global_user::GlobalUser;
use crate::settings::toml::Target;

const API_MAX_PAIRS: usize = 10000;
// The consts below are halved from the API's true capacity to help avoid
// hammering it with large requests.
pub const BATCH_KEY_MAX: usize = API_MAX_PAIRS / 2;
const UPLOAD_MAX_SIZE: usize = 50 * 1024 * 1024;

// Create a special API client that has a longer timeout than usual, given that KV operations
// can be lengthy if payloads are large.
fn bulk_api_client(user: &GlobalUser) -> Result<HttpApiClient, failure::Error> {
    let config = HttpApiClientConfig {
        http_timeout: user.get_http_config().get_bulk_timeout(),
        default_headers: headers(None),
    };

    HttpApiClient::new(
        Credentials::from(user.to_owned()),
        config,
        Environment::Production,
    )
}

pub fn put(
    target: &Target,
    user: &GlobalUser,
    namespace_id: &str,
    pairs: Vec<KeyValuePair>,
    progress_bar: &Option<ProgressBar>,
) -> Result<(), failure::Error> {
    let client = bulk_api_client(user)?;

    for b in batch_keys_values(pairs) {
        match client.request(&WriteBulk {
            account_identifier: &target.account_id,
            namespace_identifier: namespace_id,
            bulk_key_value_pairs: b.to_owned(),
        }) {
            Ok(_) => {}
            Err(e) => failure::bail!("{}", format_error(e)),
        }

        if let Some(pb) = &progress_bar {
            pb.inc(b.len() as u64);
        }
    }

    Ok(())
}

pub fn delete(
    target: &Target,
    user: &GlobalUser,
    namespace_id: &str,
    keys: Vec<String>,
    progress_bar: &Option<ProgressBar>,
) -> Result<(), failure::Error> {
    let client = bulk_api_client(user)?;

    for b in batch_keys(keys) {
        match client.request(&DeleteBulk {
            account_identifier: &target.account_id,
            namespace_identifier: namespace_id,
            bulk_keys: b.to_owned(),
        }) {
            Ok(_) => {}
            Err(e) => failure::bail!("{}", format_error(e)),
        }

        if let Some(pb) = &progress_bar {
            pb.inc(b.len() as u64);
        }
    }

    Ok(())
}

fn batch_keys_values(mut pairs: Vec<KeyValuePair>) -> Vec<Vec<KeyValuePair>> {
    let mut batches: Vec<Vec<KeyValuePair>> = Vec::new();

    if !pairs.is_empty() {
        // Iterate over all key-value pairs and create batches of uploads, each of which are
        // maximum 5K key-value pairs in size OR maximum ~50MB in size. Upload each batch
        // as it is created.
        let mut key_count = 0;
        let mut key_pair_bytes = 0;
        let mut key_value_batch: Vec<KeyValuePair> = Vec::new();

        while !(pairs.is_empty() && key_value_batch.is_empty()) {
            if pairs.is_empty() {
                // Last batch to upload
                batches.push(key_value_batch.to_vec());
                key_value_batch.clear();
            } else {
                let pair = pairs.pop().unwrap();
                if key_count + 1 > BATCH_KEY_MAX
                // Keep upload size small to keep KV bulk API happy
                || key_pair_bytes + pair.key.len() + pair.value.len() > UPLOAD_MAX_SIZE
                {
                    batches.push(key_value_batch.to_vec());
                    key_count = 0;
                    key_pair_bytes = 0;
                    key_value_batch.clear();
                }

                // Add the popped key-value pair to the running batch of key-value pair uploads
                key_count += 1;
                key_pair_bytes = key_pair_bytes + pair.key.len() + pair.value.len();
                key_value_batch.push(pair);
            }
        }
    }

    batches
}

fn batch_keys(mut keys: Vec<String>) -> Vec<Vec<String>> {
    let mut batches = Vec::new();
    while !keys.is_empty() {
        let k: Vec<String> = if keys.len() > BATCH_KEY_MAX {
            keys.drain(0..BATCH_KEY_MAX).collect()
        } else {
            keys.drain(0..).collect()
        };

        batches.push(k);
    }

    batches
}
