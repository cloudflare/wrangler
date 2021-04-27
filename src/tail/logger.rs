use crate::terminal::{emoji, styles};
use futures_util::TryStreamExt;
use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use tokio::sync::oneshot::Receiver;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, Message},
};

pub struct Logger {
    tail_id_rx: Receiver<String>,
    shutdown_rx: Receiver<()>,
    format: String,
}

/// LogServer is just a basic HTTP server running locally; it listens for POST requests on the root
/// path and simply prints the JSON body of each request as its own line to STDOUT.
impl Logger {
    pub fn new(tail_id_rx: Receiver<String>, shutdown_rx: Receiver<()>, format: String) -> Logger {
        Logger {
            tail_id_rx,
            shutdown_rx,
            format,
        }
    }

    pub async fn run(self) -> Result<(), failure::Error> {
        let tail_id = self.tail_id_rx.await?;
        let url = format!("wss://tail.developers.workers.dev/{}/ws", tail_id);
        let format = std::sync::Arc::new(self.format);

        let stream = match connect_async(url).await {
            Ok((web_socket_stream, _)) => web_socket_stream,
            Err(e) => failure::bail!(e),
        };

        let print_fut = stream.try_for_each(|message| async {
            let data = match message {
                Message::Close(_msg) => return Err(tungstenite::Error::ConnectionClosed),
                Message::Ping(_) | Message::Pong(_) => return Ok(()),
                Message::Text(text) => text,
                other => {
                    eprintln!("Unexpected message {:#?}", other);
                    return Err(tungstenite::Error::Utf8);
                }
            };

            match format.as_str() {
                "json" => print_logs_json(data),
                "pretty" => print_logs_pretty(data),
                _ => unreachable!(),
            }
            .map_err(|e| {
                eprintln!("{}", e.to_string());
                tungstenite::Error::Utf8
            })
        });

        // TODO close the socket wtf why is this impossible
        // stream.close(None).await;

        let result = tokio::select! {
            _ = self.shutdown_rx => { Ok(()) }
            result = print_fut => { result }
        };

        if result.is_err() {
            let err = result.unwrap_err();
            match err {
                tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed => {
                    // we dgaf about the error at this point from closing the stream
                    Ok(())
                }
                _other => Err(failure::err_msg(
                    "Encountered an error! There is likely additional output above",
                )),
            }
        } else {
            Ok(())
        }
    }
}

fn print_logs_json(log: String) -> Result<(), failure::Error> {
    println!("{}", log);
    Ok(())
}

fn print_logs_pretty(log: String) -> Result<(), failure::Error> {
    let parsed = serde_json::from_str::<LogResponse>(&log).map_err(|e| {
                println!("{}", styles::warning("Error parsing response body!"));
                println!(
                    "This is not a problem with your worker, it's a problem with Wrangler.\nPlease file an issue on our GitHub page, with a minimal reproducible example of\nthe script that caused this error and a description of what happened."
                );
                e
            })?;

    let secs = (parsed.event_timestamp / 1000).try_into().unwrap();

    let timestamp = chrono::NaiveDateTime::from_timestamp(secs, 0);

    println!(
        "{}{} {} --> {} @ {} UTC",
        emoji::EYES,
        parsed.event.request.method,
        styles::url(parsed.event.request.url),
        parsed.outcome.to_uppercase(),
        timestamp.time()
    );

    if !parsed.exceptions.is_empty() {
        println!("  Exceptions:");
        parsed.exceptions.iter().for_each(|exception| {
            println!(
                "\t{} {}",
                emoji::X,
                styles::warning(format!("{}: {}", exception.name, exception.message))
            );
        });
    }

    if !parsed.logs.is_empty() {
        println!("  Logs:");
        parsed.logs.iter().for_each(|log| {
            let messages = log.message.join(" ");

            let output = match log.level.as_str() {
                "assert" | "error" => format!("{} {}", emoji::X, styles::warning(messages)),
                "warn" => format!("{} {}", emoji::WARN, styles::highlight(messages)),
                "trace" | "debug" => {
                    format!("{}{}", emoji::MICROSCOPE, styles::cyan(messages))
                }
                _ => format!("{} {}", emoji::FILES, styles::bold(messages)),
            };

            println!("\t{}", output);
        });
    };

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogResponse {
    outcome: String,
    script_name: Option<String>,
    // todo: wtf gets served here
    exceptions: Vec<LogException>,
    logs: Vec<LogMessage>,
    event_timestamp: usize,
    event: RequestEvent,
}

#[derive(Debug, Serialize, Deserialize)]
struct LogException {
    name: String,
    message: String,
    timestamp: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct LogMessage {
    message: Vec<String>,
    level: String,
    timestamp: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct RequestEvent {
    request: RequestEventData,
}

#[derive(Debug, Serialize, Deserialize)]
struct RequestEventData {
    url: String,
    method: String,
    // headers: bruh,
    // cf: RequestEventCfData, // lol
}
