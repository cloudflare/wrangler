mod fiddle_messenger;
use fiddle_messenger::*;

mod http_method;
pub use http_method::HttpMethod;

mod request_payload;
pub use request_payload::RequestPayload;

mod upload;
pub use upload::upload;

use std::sync::mpsc::channel;
use std::thread;

use log::info;
use url::Url;
use ws::{Sender, WebSocket};

use crate::http;
use crate::settings::global_user::GlobalUser;
use crate::settings::toml::Target;
use crate::terminal::message::{Message, StdOut};
use crate::terminal::open_browser;
use crate::watch::watch_and_build;
use crate::{build::build_target, settings::http_config::HttpConfig};

pub fn preview(
    mut target: Target,
    user: Option<GlobalUser>,
    options: PreviewOpt,
    verbose: bool,
) -> Result<(), failure::Error> {
    build_target(&target)?;

    let sites_preview: bool = target.site.is_some();

    let script_id = upload(&mut target, user.as_ref(), sites_preview, verbose)?;

    let request_payload = RequestPayload::create(options.method, options.url, options.body);

    let session = &request_payload.session;
    let browser_url = &request_payload.browser_url;

    if options.livereload {
        // explicitly use 127.0.0.1, since localhost can resolve to 2 addresses
        let server = WebSocket::new(|out| FiddleMessageServer { out })?.bind("127.0.0.1:0")?;

        let ws_port = server.local_addr()?.port();

        info!("Opened websocket server on port {}", ws_port);

        if !options.headless {
            open_browser(&format!(
                "https://cloudflareworkers.com/?wrangler_session_id={0}&wrangler_ws_port={1}&hide_editor#{2}:{3}",
                session, ws_port, script_id, browser_url
            ))?;
        }

        // Make a the initial request to the URL
        client_request(
            &request_payload,
            &script_id,
            sites_preview,
            user.as_ref().map(|u| u.get_http_config()),
        );

        let broadcaster = server.broadcaster();
        thread::spawn(move || server.run());
        watch_for_changes(
            target,
            user.as_ref(),
            broadcaster,
            verbose,
            options.headless,
            request_payload,
        )?;
    } else {
        if !options.headless {
            open_browser(&format!(
                "https://cloudflareworkers.com/?hide_editor#{0}:{1}",
                script_id, browser_url
            ))?;
        }

        client_request(
            &request_payload,
            &script_id,
            sites_preview,
            user.as_ref().map(|u| u.get_http_config()),
        );
    }

    Ok(())
}

#[derive(Clone, Debug)]
pub struct PreviewOpt {
    pub method: HttpMethod,
    pub url: Url,
    pub body: Option<String>,
    pub livereload: bool,
    pub headless: bool,
}

fn client_request(
    payload: &RequestPayload,
    script_id: &str,
    sites_preview: bool,
    http_config: Option<&HttpConfig>,
) {
    let client = http::client(http_config);

    let method = &payload.method;
    let url = &payload.service_url;
    let body = &payload.body;

    let cookie = payload.cookie(script_id);

    let worker_res = match method {
        HttpMethod::Get => get(&url, &cookie, &client).unwrap(),
        HttpMethod::Post => post(&url, &cookie, &body, &client).unwrap(),
    };

    let msg = if sites_preview {
        "Your Worker is a Workers Site, please preview it in browser window.".to_string()
    } else {
        format!("Your Worker responded with: {}", worker_res)
    };
    StdOut::preview(&msg);
}

fn get(
    url: &str,
    cookie: &str,
    client: &reqwest::blocking::Client,
) -> Result<String, failure::Error> {
    let res = client.get(url).header("Cookie", cookie).send();
    Ok(res?.text()?)
}

fn post(
    url: &str,
    cookie: &str,
    body: &Option<String>,
    client: &reqwest::blocking::Client,
) -> Result<String, failure::Error> {
    let res = match body {
        Some(s) => client
            .post(url)
            .header("Cookie", cookie)
            .body(s.to_string())
            .send(),
        None => client.post(url).header("Cookie", cookie).send(),
    };
    let msg = format!("POST {}", url);
    StdOut::preview(&msg);
    Ok(res?.text()?)
}

fn watch_for_changes(
    mut target: Target,
    user: Option<&GlobalUser>,
    broadcaster: Sender,
    verbose: bool,
    headless: bool,
    request_payload: RequestPayload,
) -> Result<(), failure::Error> {
    let sites_preview: bool = target.site.is_some();

    let (tx, rx) = channel();
    watch_and_build(&target, Some(tx))?;

    while rx.recv().is_ok() {
        if let Ok(new_id) = upload(&mut target, user, sites_preview, verbose) {
            let script_id = new_id.to_string();

            let msg = FiddleMessage {
                session_id: request_payload.session.clone(),
                data: FiddleMessageData::LiveReload {
                    new_id: new_id.clone(),
                },
            };

            if !headless {
                match broadcaster.send(serde_json::to_string(&msg)?) {
                    Ok(_) => {
                        StdOut::preview("Updated preview with changes");
                    }
                    Err(_e) => StdOut::user_error("communication with preview failed"),
                }
            }

            client_request(
                &request_payload,
                &script_id,
                sites_preview,
                user.map(|u| u.get_http_config()),
            );
        }
    }

    broadcaster.shutdown()?;

    Ok(())
}
