mod plain_text;
mod project_assets;
mod text_blob;
mod wasm_module;

use reqwest::blocking::multipart::{Form, Part};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::settings::binding;
use crate::settings::metadata::Metadata;
use crate::settings::toml::{Target, TargetType};
use crate::sites::AssetManifest;
use crate::wranglerjs;

use plain_text::PlainText;
use project_assets::ProjectAssets;
use text_blob::TextBlob;
use wasm_module::WasmModule;

// TODO: https://github.com/cloudflare/wrangler/issues/1083
use super::{krate, Package};

pub fn build(
    target: &Target,
    asset_manifest: Option<AssetManifest>,
    session_config: Option<serde_json::Value>,
) -> Result<Form, failure::Error> {
    let target_type = &target.target_type;
    let kv_namespaces = &target.kv_namespaces;
    let mut text_blobs: Vec<TextBlob> = Vec::new();
    let mut plain_texts: Vec<PlainText> = Vec::new();
    let mut wasm_modules: Vec<WasmModule> = Vec::new();

    if let Some(blobs) = &target.text_blobs {
        for (key, blob_path) in blobs.iter() {
            let blob = fs::read_to_string(blob_path)?;
            text_blobs.push(TextBlob::new(blob, key.clone())?);
        }
    }

    if let Some(vars) = &target.vars {
        for (key, value) in vars.iter() {
            plain_texts.push(PlainText::new(key.clone(), value.clone())?)
        }
    }

    match target_type {
        TargetType::Rust => {
            log::info!("Rust project detected. Publishing...");
            let name = krate::Krate::new("./")?.name.replace("-", "_");
            // TODO: move into build?
            build_generated_dir()?;
            concat_js(&name)?;

            let path = PathBuf::from(format!("./pkg/{}_bg.wasm", name));
            let binding = "wasm".to_string();
            let wasm_module = WasmModule::new(path, binding)?;
            wasm_modules.push(wasm_module);
            let script_path = PathBuf::from("./worker/generated/script.js");

            let assets = ProjectAssets::new(
                script_path,
                wasm_modules,
                kv_namespaces.to_vec(),
                text_blobs,
                plain_texts,
            )?;

            build_form(&assets, session_config)
        }
        TargetType::JavaScript => {
            log::info!("JavaScript project detected. Publishing...");
            let mut script_path = target.build_dir()?;

            if !target.main.is_empty() {
                // use main key from wrangler.toml
                script_path.push(&target.main);

                if !std::path::Path::new(&script_path).exists() {
                    failure::bail!("The file {} does not exist. Please set a vaild file in the 'main' key of wrangler.toml", target.main);
                }
            } else {
                let build_dir = target.build_dir()?;
                let package = Package::new(&build_dir)?;

                script_path = package.main(&build_dir)?;
            }

            let assets = ProjectAssets::new(
                script_path,
                wasm_modules,
                kv_namespaces.to_vec(),
                text_blobs,
                plain_texts,
            )?;

            build_form(&assets, session_config)
        }
        TargetType::Webpack => {
            log::info!("webpack project detected. Publishing...");
            // TODO: https://github.com/cloudflare/wrangler/issues/850
            let build_dir = target.build_dir()?;
            let bundle = wranglerjs::Bundle::new(&build_dir);

            let script_path = bundle.script_path();

            if bundle.has_wasm() {
                let path = bundle.wasm_path();
                let binding = bundle.get_wasm_binding();
                let wasm_module = WasmModule::new(path, binding)?;
                wasm_modules.push(wasm_module);
            }

            if let Some(asset_manifest) = asset_manifest {
                log::info!("adding __STATIC_CONTENT_MANIFEST");
                let binding = "__STATIC_CONTENT_MANIFEST".to_string();
                let asset_manifest_blob = get_asset_manifest_blob(asset_manifest)?;
                let text_blob = TextBlob::new(asset_manifest_blob, binding)?;
                text_blobs.push(text_blob);
            }

            let assets = ProjectAssets::new(
                script_path,
                wasm_modules,
                kv_namespaces.to_vec(),
                text_blobs,
                plain_texts,
            )?;

            build_form(&assets, session_config)
        }
    }
}

fn get_asset_manifest_blob(asset_manifest: AssetManifest) -> Result<String, failure::Error> {
    let asset_manifest = serde_json::to_string(&asset_manifest)?;
    Ok(asset_manifest)
}

fn build_form(
    assets: &ProjectAssets,
    session_config: Option<serde_json::Value>,
) -> Result<Form, failure::Error> {
    let mut form = Form::new();

    // The preview service in particular streams the request form, and requires that the
    // "metadata" part be set first, so this order is important.
    form = add_metadata(form, assets)?;
    form = add_files(form, assets)?;
    if let Some(session_config) = session_config {
        form = add_session_config(form, session_config)?
    }

    log::info!("building form");
    log::info!("{:?}", &form);

    Ok(form)
}

fn add_files(mut form: Form, assets: &ProjectAssets) -> Result<Form, failure::Error> {
    form = form.file(assets.script_name(), assets.script_path())?;

    for wasm_module in &assets.wasm_modules {
        form = form.file(wasm_module.filename(), wasm_module.path())?;
    }

    for text_blob in &assets.text_blobs {
        let part = Part::text(text_blob.data.clone())
            .file_name(text_blob.binding.clone())
            .mime_str("text/plain")?;

        form = form.part(text_blob.binding.clone(), part);
    }

    Ok(form)
}

fn add_metadata(mut form: Form, assets: &ProjectAssets) -> Result<Form, failure::Error> {
    let metadata_json = serde_json::json!(&Metadata {
        body_part: assets.script_name(),
        bindings: assets.bindings(),
    });

    let metadata = Part::text((metadata_json).to_string())
        .file_name("metadata.json")
        .mime_str("application/json")?;

    form = form.part("metadata", metadata);

    Ok(form)
}

fn add_session_config(
    mut form: Form,
    session_config: serde_json::Value,
) -> Result<Form, failure::Error> {
    let wrangler_session_config = Part::text((session_config).to_string())
        .file_name("")
        .mime_str("application/json")?;

    form = form.part("wrangler-session-config", wrangler_session_config);

    Ok(form)
}

fn filename_from_path(path: &PathBuf) -> Option<String> {
    path.file_stem()?.to_str().map(|s| s.to_string())
}

fn build_generated_dir() -> Result<(), failure::Error> {
    let dir = "./worker/generated";
    if !Path::new(dir).is_dir() {
        fs::create_dir("./worker/generated")?;
    }

    Ok(())
}

fn concat_js(name: &str) -> Result<(), failure::Error> {
    let bindgen_js_path = format!("./pkg/{}.js", name);
    let bindgen_js: String = fs::read_to_string(bindgen_js_path)?.parse()?;

    let worker_js: String = fs::read_to_string("./worker/worker.js")?.parse()?;
    let js = format!("{} {}", bindgen_js, worker_js);

    fs::write("./worker/generated/script.js", js.as_bytes())?;
    Ok(())
}
