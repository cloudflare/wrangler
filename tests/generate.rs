use assert_cmd::prelude::*;

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn it_generates_with_defaults() {
    let name = "worker";
    generate(None, None, None);

    assert_eq!(Path::new(name).exists(), true);

    let wranglertoml_path = format!("{}/wrangler.toml", name);
    assert_eq!(Path::new(&wranglertoml_path).exists(), true);
    cleanup(name);
}

#[test]
fn it_generates_with_arguments() {
    let name = "example";
    let template = "https://github.com/cloudflare/rustwasm-worker-template";
    let project_type = "webpack";
    generate(Some(name), Some(template), Some(project_type));

    assert_eq!(Path::new(name).exists(), true);

    let wranglertoml_path = format!("{}/wrangler.toml", name);
    assert_eq!(Path::new(&wranglertoml_path).exists(), true);
    let wranglertoml_text = fs::read_to_string(wranglertoml_path).unwrap();
    assert!(wranglertoml_text.contains(project_type));
    cleanup(name);
}

#[test]
fn it_generates_multiple_times_with_same_name_ignoring_previous_manifest() {
    let name = "demo";
    let template = "https://github.com/cloudflare/rustwasm-worker-template";
    let project_type = "webpack";

    generate(Some(name), Some(template), Some(project_type));

    assert_eq!(Path::new(name).exists(), true);

    // replace the old manifest with a malformed one to test if a new run fails
    // by using it
    let og_manifest = Path::new(name).join("wrangler.toml");
    assert_eq!(og_manifest.exists(), true);
    fs::write(&og_manifest, "{% nope %}").unwrap();

    generate(Some(name), Some(template), Some(project_type));

    let new_name = "demo-1";

    assert_eq!(Path::new(new_name).exists(), true);

    cleanup(name);
    cleanup(new_name);
}

pub fn generate(name: Option<&str>, template: Option<&str>, project_type: Option<&str>) {
    let mut wrangler = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    if name.is_none() && template.is_none() && project_type.is_none() {
        wrangler.arg("generate").assert().success();
    } else if name.is_some() && template.is_some() && project_type.is_some() {
        wrangler
            .arg("generate")
            .arg(name.unwrap())
            .arg(template.unwrap())
            .arg("--type")
            .arg(project_type.unwrap())
            .assert()
            .success();
    }
}

fn cleanup(name: &str) {
    fs::remove_dir_all(name).unwrap();
    assert_eq!(Path::new(name).exists(), false);
}
