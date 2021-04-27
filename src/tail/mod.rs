///! `wrangler tail` allows Workers users to collect logs from their deployed Workers.
///! When a user runs `wrangler tail`, several things happen:
///!     1. Wrangler creates a tail by making a request to the Workers API /tail endpoint
///!     2. The Workers API creates a tail ID and binds it to a [Trace Worker], and directs all `console` and
///!        exception logging to the Trace Worker. It then returns the tail ID to wrangler.
///!        - The trace worker consists of a handler for the logging inputs and a [Durable Object]. The handler
///!          forwards logs to the Durable Object, which then forwards logs (via websocket) to any attached listeners
///!     3. Wrangler takes the returned tail ID, and instantiates a websocket connection to the trace worker('s durable object)
///!     4. When a message comes across from the WebSocket, wrangler prints the output to STDOUT (formatted according to `--format`)
mod logger;
mod session;
mod shutdown;

use logger::Logger;
use session::Session;
use shutdown::ShutdownHandler;

use tokio::runtime::Runtime as TokioRuntime;

use crate::settings::global_user::GlobalUser;
use crate::settings::toml::Target;
use crate::terminal::emoji;

pub struct Tail;

impl Tail {
    pub fn run(
        target: &Target,
        user: &GlobalUser,
        format: String,
        verbose: bool,
    ) -> Result<(), failure::Error> {
        // Note that we use eprintln!() throughout this module; this is because we want any
        // helpful output to not be mixed with actual log JSON output, so we use this macro
        // to print messages to stderr instead of stdout (where log output is printed).
        eprintln!(
            "{} Setting up log streaming from Worker script \"{}\".",
            emoji::TAIL,
            target.name,
        );

        let mut runtime = TokioRuntime::new()?;

        runtime.block_on(async {
            // TODO: replace below with https://detegr.github.io/doc/ctrlc/
            // Create two [one-shot](https://docs.rs/tokio/0.2.16/tokio/sync#oneshot-channel)
            // channels for handling ctrl-c. Each channel has two parts:
            // tx: Transmitter
            // rx: Receiver
            let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel(); // shutdown short circuit
            let mut shutdown_handler = ShutdownHandler::new();
            let session_shutdown_rx = shutdown_handler.subscribe();
            let log_shutdown_rx = shutdown_handler.subscribe();

            // Create a one-shot for passing the Tail ID between the Session and the Logger
            let (tail_id_tx, tail_id_rx) = tokio::sync::oneshot::channel();

            let shutdown_listener = tokio::spawn(shutdown_handler.run(shutdown_rx));
            let logger = tokio::spawn(Logger::new(tail_id_rx, log_shutdown_rx, format).run());

            // Register the tail with the Workers API and send periodic heartbeats
            let session = tokio::spawn(Session::run(
                target.clone(),
                user.clone(),
                session_shutdown_rx,
                shutdown_tx,
                tail_id_tx,
                verbose,
            ));

            let res = tokio::try_join!(
                async { shutdown_listener.await? },
                async { session.await? },
                async { logger.await? },
            );

            match res {
                Ok(_) => Ok(()),
                Err(e) => Err(e),
            }
        })
    }
}
