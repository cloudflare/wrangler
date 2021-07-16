use std::env;
use std::path::Path;

use anyhow::Result;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};

use crate::build::build_target;
use crate::deploy::{self, DeploymentSet};
use crate::http::{self, Feature};
use crate::kv::bulk;
use crate::settings::global_user::GlobalUser;
use crate::settings::toml::migrations::{MigrationTag, Migrations};
use crate::settings::toml::Target;
use crate::sites;
use crate::terminal::emoji;
use crate::terminal::message::{Message, Output, StdErr, StdOut};
use crate::upload;

#[derive(Serialize, Deserialize, Default)]
pub struct PublishOutput {
    pub success: bool,
    pub name: String,
    pub urls: Vec<String>,
    pub schedules: Vec<String>,
}

pub fn publish(
    user: &GlobalUser,
    target: &mut Target,
    deployments: DeploymentSet,
    out: Output,
) -> Result<()> {
    validate_target_required_fields_present(target)?;

    let run_deploy = |target: &Target| match deploy::deploy(&user, &deployments) {
        Ok(results) => {
            build_output_message(results, target.name.clone(), out);
            Ok(())
        }
        Err(e) => Err(e),
    };

    // Build the script before uploading and log build result
    let build_result = build_target(&target);
    match build_result {
        Ok(msg) => {
            StdErr::success(&msg);
            Ok(())
        }
        Err(e) => Err(e),
    }?;

    if let Some(build_config) = &target.build {
        build_config.verify_upload_dir()?;
    }

    if target.migrations.is_some() {
        // Can't do this in the if below, since that one takes a mutable borrow on target
        let client = http::legacy_auth_client(user);
        let script_migration_tag = get_migration_tag(&client, target)?;

        match target.migrations.as_mut().unwrap() {
            Migrations::Adhoc { script_tag, .. } => *script_tag = script_migration_tag,
            Migrations::List { script_tag, .. } => *script_tag = script_migration_tag,
        };
    }

    if let Some(site_config) = &target.site {
        let path = &site_config.bucket.clone();
        validate_bucket_location(path)?;

        let site_namespace = sites::add_namespace(user, target, false)?;

        let (to_upload, to_delete, asset_manifest) =
            sites::sync(target, user, &site_namespace.id, &path)?;

        // First, upload all existing files in bucket directory
        StdErr::working("Uploading site files");
        let upload_progress_bar = if to_upload.len() > bulk::BATCH_KEY_MAX {
            let upload_progress_bar = ProgressBar::new(to_upload.len() as u64);
            upload_progress_bar
                .set_style(ProgressStyle::default_bar().template("{wide_bar} {pos}/{len}\n{msg}"));
            Some(upload_progress_bar)
        } else {
            None
        };

        bulk::put(
            target,
            user,
            &site_namespace.id,
            to_upload,
            &upload_progress_bar,
        )?;

        if let Some(pb) = upload_progress_bar {
            pb.finish_with_message("Done Uploading");
        }

        let upload_client = http::featured_legacy_auth_client(user, Feature::Sites);

        // Next, upload and deploy the worker with the updated asset_manifest
        upload::script(&upload_client, &target, Some(asset_manifest))?;

        run_deploy(target)?;

        // Finally, remove any stale files
        if !to_delete.is_empty() {
            StdErr::info("Deleting stale files...");

            let delete_progress_bar = if to_delete.len() > bulk::BATCH_KEY_MAX {
                let delete_progress_bar = ProgressBar::new(to_delete.len() as u64);
                delete_progress_bar.set_style(
                    ProgressStyle::default_bar().template("{wide_bar} {pos}/{len}\n{msg}"),
                );
                Some(delete_progress_bar)
            } else {
                None
            };

            bulk::delete(
                target,
                user,
                &site_namespace.id,
                to_delete,
                &delete_progress_bar,
            )?;

            if let Some(pb) = delete_progress_bar {
                pb.finish_with_message("Done deleting");
            }
        }
    } else {
        let upload_client = http::legacy_auth_client(user);

        upload::script(&upload_client, &target, None)?;
        run_deploy(target)?;
    }

    Ok(())
}

fn build_output_message(deploy_results: deploy::DeployResults, target_name: String, out: Output) {
    let deploy::DeployResults { urls, schedules } = deploy_results;

    let mut msg = "Successfully published your script ".to_owned();
    if !urls.is_empty() {
        msg.push_str(&format!("to\n {}\n", urls.join("\n ")));
    }
    if !schedules.is_empty() {
        msg.push_str(&format!("with this schedule\n {}\n", schedules.join("\n ")));
    }

    StdErr::success(&msg);
    if out == Output::Json {
        StdOut::as_json(&PublishOutput {
            success: true,
            name: target_name,
            urls,
            schedules,
        });
    }
}

// We don't want folks setting their bucket to the top level directory,
// which is where wrangler commands are always called from.
pub fn validate_bucket_location(bucket: &Path) -> Result<()> {
    // TODO: this should really use a convenience function for "Wrangler Project Root"
    let current_dir = env::current_dir()?;
    if bucket.as_os_str() == current_dir {
        anyhow::bail!(
            "{} Your bucket cannot be set to the parent directory of your configuration file",
            emoji::WARN
        )
    }
    let path = Path::new(&bucket);
    if !path.exists() {
        anyhow::bail!(
            "{} bucket directory \"{}\" does not exist",
            emoji::WARN,
            path.display()
        )
    } else if !path.is_dir() {
        anyhow::bail!(
            "{} bucket \"{}\" is not a directory",
            emoji::WARN,
            path.display()
        )
    }

    Ok(())
}

fn validate_target_required_fields_present(target: &Target) -> Result<()> {
    let mut missing_fields = Vec::new();

    if target.name.is_empty() {
        missing_fields.push("name")
    };

    for kv in &target.kv_namespaces {
        if kv.binding.is_empty() {
            missing_fields.push("kv-namespace binding")
        }

        if kv.id.is_empty() {
            missing_fields.push("kv-namespace id")
        }
    }

    let (field_pluralization, is_are) = match missing_fields.len() {
        n if n >= 2 => ("fields", "are"),
        1 => ("field", "is"),
        _ => ("", ""),
    };

    if !missing_fields.is_empty() {
        anyhow::bail!(
            "{} Your configuration file is missing the {} {:?} which {} required to publish your worker!",
            emoji::WARN,
            field_pluralization,
            missing_fields,
            is_are,
        );
    };

    Ok(())
}

fn get_migration_tag(client: &Client, target: &Target) -> Result<MigrationTag, anyhow::Error> {
    // Today, the easiest way to get metadata about a script (including the migration tag)
    // is the list endpoint, as the individual script endpoint just returns the source code for a
    // given script (and doesn't work at all for DOs). Once we add an individual script metadata
    // endpoint, we could use that here instead of listing all of the scripts. Listing isn't too bad
    // today though, as most accounts are limited to 30 scripts anyways.

    let addr = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/workers/scripts",
        target.account_id.load()?
    );

    let res: ListScriptsV4ApiResponse = client.get(&addr).send()?.json()?;

    let tag = match res.result.into_iter().find(|s| s.id == target.name) {
        Some(ScriptResponse {
            migration_tag: Some(tag),
            ..
        }) => MigrationTag::HasTag(tag),
        Some(ScriptResponse {
            migration_tag: None,
            ..
        }) => MigrationTag::NoTag,
        None => MigrationTag::NoScript,
    };

    log::info!("Current MigrationTag: {:#?}", tag);

    Ok(tag)
}

#[derive(Debug, Deserialize)]
struct ListScriptsV4ApiResponse {
    pub result: Vec<ScriptResponse>,
}

#[derive(Debug, Deserialize)]
struct ScriptResponse {
    pub id: String,
    pub migration_tag: Option<String>,
}
