use std::fs::File;


use anyhow::Result;
use reqwest::blocking::multipart::{Form, Part};
use serde::Serialize;

use crate::settings::binding::Binding;
use crate::settings::toml::migrations::ApiMigration;

use super::{ModulesAssets, UsageModel};

#[derive(Serialize, Debug)]
struct Metadata {
    pub main_module: String,
    pub bindings: Vec<Binding>,
    pub migrations: Option<ApiMigration>,
    pub usage_model: Option<UsageModel>,
}

pub fn build_form(
    assets: &ModulesAssets,
    session_config: Option<serde_json::Value>
) -> Result<Form> {
    let mut form = Form::new();

    // The preview service in particular streams the request form, and requires that the
    // "metadata" part be set first, so this order is important.
    form = add_metadata(form, assets)?;
    form = add_files(form, assets)?;

    if let Some(session_config) = session_config {
        form = add_session_config(form, session_config)?
    }

    log::info!("building form");
    log::info!("{:#?}", &form);

    Ok(form)
}

pub fn build_form_with_manifest(
    assets: &ModulesAssets,
    session_config: Option<serde_json::Value>,
    static_manifest: String,
) -> Result<Form> {
    let mut form = Form::new();

    // The preview service in particular streams the request form, and requires that the
    // "metadata" part be set first, so this order is important.
    form = add_metadata(form, assets)?;
    form = add_files(form, assets)?;

    form = form.part(
        "STATIC_CONTENT_MANIFEST",
        Part::text(static_manifest)
            .mime_str("text/plain")?
            .file_name("STATIC_CONTENT_MANIFEST.txt"),
    );

    if let Some(session_config) = session_config {
        form = add_session_config(form, session_config)?
    }

    log::info!("building form");
    log::info!("{:#?}", &form);

    Ok(form)
}

fn add_files(mut form: Form, assets: &ModulesAssets) -> Result<Form> {
    for (name, module) in &assets.manifest.modules {
        let part = Part::reader(File::open(module.path.clone())?)
            .mime_str(module.module_type.content_type())?
            .file_name(name.clone());
        form = form.part(name.clone(), part);
    }

    Ok(form)
}

fn add_metadata(mut form: Form, assets: &ModulesAssets) -> Result<Form> {
    let metadata_json = serde_json::json!(&Metadata {
        main_module: assets.manifest.main.clone(),
        bindings: assets.bindings(),
        migrations: assets.migration.clone(),
        usage_model: assets.usage_model,
    });

    let metadata = Part::text(metadata_json.to_string())
        .file_name("metadata.json")
        .mime_str("application/json")?;

    form = form.part("metadata", metadata);

    Ok(form)
}

fn add_session_config(mut form: Form, session_config: serde_json::Value) -> Result<Form> {
    let wrangler_session_config = Part::text(session_config.to_string())
        .file_name("")
        .mime_str("application/json")?;

    form = form.part("wrangler-session-config", wrangler_session_config);

    Ok(form)
}
