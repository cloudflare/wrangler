use super::AdhocMigration;
use super::Cli;
use crate::commands;
use crate::settings::{global_user::GlobalUser, toml::Manifest};
use crate::terminal::message::{Message, Output, StdOut};
use crate::terminal::styles;

use anyhow::Result;

pub fn publish(
    release: bool,
    output: Option<String>,
    migration: AdhocMigration,
    cli_params: &Cli,
) -> Result<()> {
    log::info!("Getting User settings");
    let user = GlobalUser::new()?;

    if release {
        StdOut::warn(&format!(concat!(
            "{} is deprecated and behaves exactly the same as {}.\n",
            "See {} for more information."),
            styles::highlight("`wrangler publish --release`"),
            styles::highlight("`wrangler publish`"),
            styles::url("https://developers.cloudflare.com/workers/tooling/wrangler/configuration/environments"),
        ));
    }

    log::info!("Getting project settings");
    let manifest = Manifest::new(&cli_params.config)?;
    let mut target = manifest.get_target(cli_params.environment.as_deref(), false)?;

    if let Some(migration) = migration.into_migrations() {
        target.migrations = Some(migration);
    }

    let output = if output.as_deref() == Some("json") {
        Output::Json
    } else {
        Output::PlainText
    };
    let deploy_config = manifest.get_deployments(cli_params.environment.as_deref())?;
    commands::publish(&user, &mut target, deploy_config, output)
}
