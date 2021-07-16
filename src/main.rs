#![cfg_attr(feature = "strict", deny(warnings))]

extern crate text_io;
extern crate tokio;

use std::env;

use wrangler::cli::{exec, Cli, Command};
use wrangler::commands;
use wrangler::installer;
use wrangler::reporter;
use wrangler::terminal::message::{Message, StdOut};
use wrangler::terminal::styles;
use wrangler::version::background_check_for_updates;

use anyhow::Result;
use structopt::StructOpt;

fn main() -> Result<()> {
    reporter::init();
    env_logger::init();

    let latest_version_receiver = background_check_for_updates();
    if let Ok(me) = env::current_exe() {
        // If we're actually running as the installer then execute our
        // self-installation, otherwise just continue as usual.
        if me
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("executable should have a filename")
            .starts_with("wrangler-init")
        {
            installer::install()?;
        }
    }
    run()?;
    if let Ok(latest_version) = latest_version_receiver.try_recv() {
        let latest_version = styles::highlight(latest_version.to_string());
        let new_version_available = format!(
            "A new version of Wrangler ({}) is available!",
            latest_version
        );
        let update_message = "You can learn more about updating here:".to_string();
        let update_docs_url = styles::url(
            "https://developers.cloudflare.com/workers/cli-wrangler/install-update#update",
        );

        StdOut::billboard(&format!(
            "{}\n{}\n{}",
            new_version_available, update_message, update_docs_url
        ));
    }
    Ok(())
}

fn run() -> Result<()> {
    let cli = Cli::from_args();
    let cli_params = cli.clone();

    match cli.command {
        Command::Config { api_key, no_verify } => exec::configure(api_key, no_verify),
        Command::Generate {
            name,
            site,
            template,
            target_type,
        } => exec::generate(name, site, template, target_type),
        Command::Init {
            name,
            site,
            target_type,
        } => exec::init(name, site, target_type),
        Command::Build => exec::build(&cli_params),
        Command::Preview {
            method,
            url,
            body,
            watch,
            headless,
        } => exec::preview(method, url, body, watch, headless, &cli_params),
        Command::Dev {
            host,
            ip,
            port,
            local_protocol,
            upstream_protocol,
        } => exec::dev(
            host,
            ip,
            port,
            local_protocol,
            upstream_protocol,
            &cli_params,
        ),
        Command::Whoami => exec::whoami(),
        Command::Publish {
            release,
            output,
            migration,
        } => exec::publish(release, output, migration, &cli_params),
        Command::Subdomain { name } => exec::subdomain(name, &cli_params),
        Command::Route(route) => exec::route(route, &cli_params),
        Command::Secret(secret) => exec::secret(secret, &cli_params),
        Command::KvNamespace(namespace) => exec::kv_namespace(namespace, &cli_params),
        Command::KvKey(key) => exec::kv_key(key, &cli_params),
        Command::KvBulk(bulk) => exec::kv_bulk(bulk, &cli_params),
        Command::Tail {
            format,
            tunnel_port,
            metrics_port,
        } => exec::tail(format, tunnel_port, metrics_port, &cli_params),
        Command::Login => commands::login::run(),
        Command::Report { log } => commands::report::run(log.as_deref()).map(|_| {
            eprintln!("Report submission sucessful. Thank you!");
        }),
        Command::Zone { zone } => exec::zone(zone),
    }
}
