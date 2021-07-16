mod modules_worker;
mod plain_text;
mod project_assets;
mod service_worker;
mod text_blob;
mod wasm_module;

use anyhow::Result;
use reqwest::blocking::multipart::Form;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use crate::settings::binding;
use crate::settings::toml::{Target, TargetType, UploadFormat, UsageModel};
use crate::sites::AssetManifest;
use crate::wranglerjs;

use plain_text::PlainText;
pub use project_assets::{ModuleConfig, ModuleType};
use project_assets::{ModulesAssets, ServiceWorkerAssets};
use text_blob::TextBlob;
use wasm_module::WasmModule;

// TODO: https://github.com/cloudflare/wrangler/issues/1083
use super::{krate, Package};

pub fn build(
    target: &Target,
    asset_manifest: Option<AssetManifest>,
    session_config: Option<serde_json::Value>,
) -> Result<Form> {
    let target_type = &target.target_type;
    let kv_namespaces = &target.kv_namespaces;
    let durable_object_classes = target
        .durable_objects
        .as_ref()
        .and_then(|d| d.classes.clone())
        .unwrap_or_default();
    let mut text_blobs: Vec<TextBlob> = Vec::new();
    let mut plain_texts: Vec<PlainText> = Vec::new();
    let mut wasm_modules: Vec<WasmModule> = Vec::new();
    let usage_model = target.usage_model;

    if let Some(blobs) = &target.text_blobs {
        for (key, blob_path) in blobs.iter() {
            let blob = fs::read_to_string(blob_path)?;
            text_blobs.push(TextBlob::new(blob, key.clone())?);
        }
    }

    if let Some(modules) = &target.wasm_modules {
        for (key, module_path) in modules.iter() {
            wasm_modules.push(WasmModule::new(module_path.clone(), key.clone())?);
        }
    }

    if let Some(vars) = &target.vars {
        for (key, value) in vars.iter() {
            plain_texts.push(PlainText::new(key.clone(), value.clone())?)
        }
    }

    if let Some(asset_manifest) = asset_manifest {
        log::info!("adding __STATIC_CONTENT_MANIFEST");
        let binding = "__STATIC_CONTENT_MANIFEST".to_string();
        let asset_manifest_blob = get_asset_manifest_blob(asset_manifest)?;
        let text_blob = TextBlob::new(asset_manifest_blob, binding)?;
        text_blobs.push(text_blob);
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

            let assets = ServiceWorkerAssets::new(
                script_path,
                wasm_modules,
                kv_namespaces.to_vec(),
                durable_object_classes,
                text_blobs,
                plain_texts,
                usage_model,
            )?;

            service_worker::build_form(&assets, session_config)
        }
        TargetType::JavaScript => match &target.build {
            Some(config) => match &config.upload {
                UploadFormat::ServiceWorker {} => {
                    log::info!("Plain JavaScript project detected. Publishing...");
                    let package_dir = target.package_dir()?;
                    let package = Package::new(&package_dir)?;
                    let script_path = package_dir.join(package.main(&package_dir)?);

                    let assets = ServiceWorkerAssets::new(
                        script_path,
                        wasm_modules,
                        kv_namespaces.to_vec(),
                        durable_object_classes,
                        text_blobs,
                        plain_texts,
                        usage_model,
                    )?;

                    service_worker::build_form(&assets, session_config)
                }
                UploadFormat::Modules { main, dir, rules } => {
                    let migration = match &target.migrations {
                        Some(migrations) => migrations.api_migration()?,
                        None => None,
                    };

                    let module_config = ModuleConfig::new(main, dir, rules);
                    let assets = ModulesAssets::new(
                        module_config.get_modules()?,
                        kv_namespaces.to_vec(),
                        durable_object_classes,
                        migration,
                        plain_texts,
                        usage_model,
                    )?;

                    modules_worker::build_form(&assets, session_config)
                }
            },
            None => {
                log::info!("Plain JavaScript project detected. Publishing...");
                let package_dir = target.package_dir()?;
                let package = Package::new(&package_dir)?;
                let script_path = package.main(&package_dir)?;

                let assets = ServiceWorkerAssets::new(
                    script_path,
                    wasm_modules,
                    kv_namespaces.to_vec(),
                    durable_object_classes,
                    text_blobs,
                    plain_texts,
                    usage_model,
                )?;

                service_worker::build_form(&assets, session_config)
            }
        },
        TargetType::Webpack => {
            log::info!("webpack project detected. Publishing...");
            // TODO: https://github.com/cloudflare/wrangler/issues/850
            let package_dir = target.package_dir()?;
            let bundle = wranglerjs::Bundle::new(&package_dir);

            let script_path = bundle.script_path();

            if bundle.has_wasm() {
                let path = bundle.wasm_path();
                let binding = bundle.get_wasm_binding();
                let wasm_module = WasmModule::new(path, binding)?;
                wasm_modules.push(wasm_module);
            }

            let assets = ServiceWorkerAssets::new(
                script_path,
                wasm_modules,
                kv_namespaces.to_vec(),
                durable_object_classes,
                text_blobs,
                plain_texts,
                usage_model,
            )?;

            service_worker::build_form(&assets, session_config)
        }
    }
}

fn get_asset_manifest_blob(asset_manifest: AssetManifest) -> Result<String> {
    let asset_manifest = serde_json::to_string(&asset_manifest)?;
    Ok(asset_manifest)
}

fn filestem_from_path(path: &Path) -> Option<String> {
    path.file_stem()?.to_str().map(|s| s.to_string())
}

fn build_generated_dir() -> Result<()> {
    let dir = "./worker/generated";
    if !Path::new(dir).is_dir() {
        fs::create_dir("./worker/generated")?;
    }

    Ok(())
}

fn concat_js(name: &str) -> Result<()> {
    let bindgen_js_path = format!("./pkg/{}.js", name);
    let bindgen_js: String = fs::read_to_string(bindgen_js_path)?.parse()?;

    let worker_js: String = fs::read_to_string("./worker/worker.js")?.parse()?;
    let js = format!("{} {}", bindgen_js, worker_js);

    fs::write("./worker/generated/script.js", js.as_bytes())?;
    Ok(())
}
