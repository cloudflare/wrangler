use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

use crate::commands::validate_worker_name;
use crate::settings::toml::{Manifest, Site, TargetType};
use crate::{commands, install};

pub fn generate(
    name: &str,
    template: &str,
    branch: Option<&str>,
    target_type: Option<TargetType>,
    site: bool,
) -> Result<(), failure::Error> {
    validate_worker_name(name)?;

    let new_name = if directory_exists(name).unwrap_or(true) {
        match generate_name(name) {
            Ok(val) => val,
            Err(_) => {
                log::debug!(
                    "Failed to auto-increment name for a new worker project, using '{}'",
                    name
                );
                String::from(name)
            }
        }
    } else {
        String::from(name)
    };

    log::info!("Generating a new worker project with name '{}'", new_name);
    run_generate(&new_name, template, branch)?;

    let config_path = PathBuf::from("./").join(&name);
    // TODO: this is tightly coupled to our site template. Need to remove once
    // we refine our generate logic.
    let generated_site = if site {
        Some(Site::new("./public"))
    } else {
        None
    };
    Manifest::generate(new_name, target_type, &config_path, generated_site)?;

    Ok(())
}

pub fn run_generate(
    name: &str,
    template: &str,
    optional_branch: Option<&str>,
) -> Result<(), failure::Error> {
    let binary_path = install::install_cargo_generate()?;
    let args = if let Some(branch) = optional_branch {
        [
            "generate", "--git", template, "--branch", branch, "--name", name, "--force",
        ]
    } else {
        [
            "generate", "--git", template, "", "", "--name", name, "--force",
        ]
    };

    let command = command(binary_path, &args);
    let command_name = format!("{:?}", command);
    commands::run(command, &command_name)
}

fn generate_name(name: &str) -> Result<String, failure::Error> {
    let mut num = 1;
    let entry_names = read_current_dir()?;
    let mut new_name = construct_name(&name, num);

    while entry_names.contains(&OsString::from(&new_name)) {
        num += 1;
        new_name = construct_name(&name, num);
    }
    Ok(new_name)
}

fn read_current_dir() -> Result<Vec<OsString>, failure::Error> {
    let current_dir = env::current_dir()?;
    let mut entry_names = Vec::new();

    for entry in fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            entry_names.push(entry.file_name());
        }
    }
    Ok(entry_names)
}

fn directory_exists(dirname: &str) -> Result<bool, failure::Error> {
    let entry_names = read_current_dir()?;
    Ok(entry_names.contains(&OsString::from(dirname)))
}

fn construct_name(name: &str, num: i32) -> String {
    format!("{}-{}", name, num)
}

fn command(binary_path: PathBuf, args: &[&str]) -> Command {
    let mut c = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.arg("/C");
        c.arg(binary_path);
        c
    } else {
        Command::new(binary_path)
    };

    c.args(args);
    c
}
