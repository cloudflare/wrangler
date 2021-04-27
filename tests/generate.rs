use assert_cmd::prelude::*;

use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn it_generates_with_defaults() {
    let name = "worker";
    generate(None, None, None, None);

    assert_eq!(Path::new(name).exists(), true);

    let wranglertoml_path = format!("{}/wrangler.toml", name);
    assert_eq!(Path::new(&wranglertoml_path).exists(), true);
    cleanup(name);
}

#[test]
fn it_generates_with_some_arguments() {
    let name = "example-rust";
    let template = "https://github.com/cloudflare/rustwasm-worker-template";

    generate(Some(name), Some(template), None, None);

    assert_eq!(Path::new(name).exists(), true);

    let wranglertoml_path = format!("{}/wrangler.toml", name);
    assert_eq!(Path::new(&wranglertoml_path).exists(), true);
    let wranglertoml_text = fs::read_to_string(wranglertoml_path).unwrap();
    assert!(wranglertoml_text.contains("type = \"rust\""));
    cleanup(name);
}

#[test]
fn it_generates_with_all_arguments() {
    let name = "example-branch";
    let template = "p-j/worker-eapi-template";
    let branch = "main";
    let project_type = "webpack";
    generate(Some(name), Some(template), Some(branch), Some(project_type));

    assert_eq!(Path::new(name).exists(), true);

    let wranglertoml_path = format!("{}/wrangler.toml", name);
    assert_eq!(Path::new(&wranglertoml_path).exists(), true);
    let wranglertoml_text = fs::read_to_string(wranglertoml_path).unwrap();
    assert!(wranglertoml_text.contains(project_type));
    cleanup(name);
}

pub fn generate(
    name: Option<&str>,
    template: Option<&str>,
    branch: Option<&str>,
    project_type: Option<&str>,
) {
    let mut wrangler = Command::cargo_bin(env!("CARGO_PKG_NAME")).unwrap();
    if name.is_none() && template.is_none() && branch.is_none() && project_type.is_none() {
        wrangler.arg("generate").assert().success();
    } else if name.is_some() && template.is_some() && branch.is_none() && project_type.is_none() {
        wrangler
            .arg("generate")
            .arg(name.unwrap())
            .arg(template.unwrap())
            .assert()
            .success();
    } else if name.is_some() && template.is_some() && branch.is_some() && project_type.is_some() {
        wrangler
            .arg("generate")
            .arg(name.unwrap())
            .arg(template.unwrap())
            .arg("--branch")
            .arg(branch.unwrap())
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
