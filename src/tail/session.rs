use std::str;
use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};
use tokio::sync::oneshot::{Receiver, Sender};
use tokio::time::delay_for;

use cloudflare::endpoints::workers::{CreateTail, DeleteTail, SendTailHeartbeat};
use cloudflare::framework::HttpApiClientConfig;
use cloudflare::framework::{async_api, async_api::ApiClient};

use crate::http;
use crate::settings::global_user::GlobalUser;
use crate::settings::toml::Target;

const KEEP_ALIVE_INTERVAL: u64 = 15;

pub struct Session {
    tail_id: Option<String>,
}

/// Session is responsible for interacting with the Workers API to establish and maintain the tail
/// connection; once the tail session is established, it uses the returned tail_id to send periodic
/// "keepalive" heartbeats to the service. If the service does not receive a heartbeat for ~10
/// minutes, it will kill the tail session by removing the trace worker.
impl Session {
    pub async fn run(
        target: Target,
        user: GlobalUser,
        shutdown_rx: Receiver<()>,
        shutdown_tx: Sender<()>,
        tail_id_tx: Sender<String>,
        verbose: bool,
    ) -> Result<(), failure::Error> {
        // During the start process we'll populate the tail with the response from the API.
        let mut session = Session { tail_id: None };
        // We need to exit on a shutdown command without waiting for API calls to complete.
        tokio::select! {
            _ = shutdown_rx => { session.close(&user, &target).await }
            result = session.start(&target, &user, shutdown_tx, tail_id_tx, verbose) => { result }
        }
    }

    async fn close(&self, user: &GlobalUser, target: &Target) -> Result<(), failure::Error> {
        // The API will clean up tails after about 10 minutes of inactivity, or 24 hours of
        // activity but since we limit the number of tails allowed on a single script we should at
        // least try to delete them as we go.
        if let Some(tail_id) = &self.tail_id {
            let client = http::cf_v4_api_client_async(&user, HttpApiClientConfig::default())?;
            client
                .request(&DeleteTail {
                    account_identifier: &target.account_id,
                    script_name: &target.name,
                    tail_id: &tail_id,
                })
                .await?;
        }

        Ok(())
    }

    async fn start(
        &mut self,
        target: &Target,
        user: &GlobalUser,
        shutdown_tx: Sender<()>,
        tail_id_tx: Sender<String>,
        verbose: bool,
    ) -> Result<(), failure::Error> {
        let style = ProgressStyle::default_spinner().template("{spinner}   {msg}");
        let spinner = ProgressBar::new_spinner().with_style(style);

        // Verbose output and the spinner don't play well together.
        if verbose {
            eprintln!("This may take a few seconds...");
        } else {
            spinner.set_message("This may take a few seconds...");
            spinner.enable_steady_tick(20);
        }

        let client = http::cf_v4_api_client_async(user, HttpApiClientConfig::default())?;

        let response = client
            .request(&CreateTail {
                account_identifier: &target.account_id,
                script_name: &target.name,
                params: Default::default(),
            })
            .await;

        match response {
            Ok(success) => {
                let tail_id = success.result.id;
                self.tail_id = Some(tail_id.clone());
                tail_id_tx
                    .send(tail_id.clone())
                    .map_err(|_| failure::err_msg("Failed to send the tail ID to the logger!"))?;

                spinner.abandon_with_message("Now prepared to stream logs.");

                // Loop indefinitely to send "heartbeat" to API and keep log streaming alive.
                // This should loop forever until SIGINT is issued or Wrangler process is killed
                // through other means.
                let duration = Duration::from_millis(1000 * KEEP_ALIVE_INTERVAL);
                let mut delay = delay_for(duration);

                loop {
                    delay.await;
                    let heartbeat_result = send_heartbeat(&target, &client, &tail_id).await;
                    if heartbeat_result.is_err() {
                        return heartbeat_result;
                    }
                    delay = delay_for(duration);
                }
            }
            Err(e) => {
                shutdown_tx.send(()).unwrap();
                failure::bail!(http::format_error(e, Some(&tail_help)))
            }
        }
    }
}

async fn send_heartbeat(
    target: &Target,
    client: &async_api::Client,
    tail_id: &str,
) -> Result<(), failure::Error> {
    let response = client
        .request(&SendTailHeartbeat {
            account_identifier: &target.account_id,
            script_name: &target.name,
            tail_id,
        })
        .await;

    match response {
        Ok(_) => Ok(()),
        Err(e) => failure::bail!(http::format_error(e, Some(&tail_help))),
    }
}

// tail_help() provides more detailed explanations of Workers KV API error codes.
// See https://api.cloudflare.com/#workers-kv-namespace-errors for details.
fn tail_help(error_code: u16) -> &'static str {
    match error_code {
        7003 | 7000 => {
            "Your configuration file is likely missing the field \"account_id\", which is required to tail a worker."
        }
        // unauthorized
        10000 => {
            "Make sure your API token has permission to both edit and read workers on your account"
        }
        // script not found
        10007 => "wrangler can only tail live Worker scripts. Run `wrangler publish` before attempting to tail.", // key errors
        // limit errors
        10057 | 10058 | 10059 => "See documentation",
        _ => "",
    }
}
