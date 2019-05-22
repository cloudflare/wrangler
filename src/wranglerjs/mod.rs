use crate::commands::publish::package::Package;
use log::info;
use number_prefix::{NumberPrefix, Prefixed, Standalone};
use serde::Deserialize;
use std::env;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::{Path, PathBuf};
use std::process::Command;

// This structure represents the communication between {wrangler-js} and
// {wrangler}. It is send back after {wrangler-js} completion.
// FIXME(sven): make this private
#[derive(Deserialize, Debug)]
pub struct WrangerjsOutput {
    wasm: Option<String>,
    wasm_name: String,
    script: String,
    // {wrangler-js} will send us the path to the {dist} directory that {Webpack}
    // used; it's tedious to remove a directory with content in JavaScript so
    // let's do it in Rust!
    dist_to_clean: String,
    // indication of file sizes
    wasm_size: f32,
    script_size: f32,
}

impl WrangerjsOutput {}

// Directory where we should write the {Bundle}. It represents the built
// artefact.
const BUNDLE_OUT: &str = "./worker";
pub struct Bundle {}

// We call a {Bundle} the output of a {Bundler}; representing what {Webpack}
// produces.
impl Bundle {
    pub fn new() -> Bundle {
        Bundle {}
    }

    pub fn write(&self, wranglerjs_output: WrangerjsOutput) -> Result<(), failure::Error> {
        let mut metadata_file = File::create(self.metadata_path())?;
        metadata_file.write_all(create_metadata(self).as_bytes())?;

        let mut script_file = File::create(self.script_path())?;
        let mut script = create_prologue();

        match wranglerjs_output.wasm {
            Some(wasm) => {
                let mut wasm_file = File::create(self.wasm_path())?;
                wasm_file.write_all(wasm.as_bytes())?;

                script +=
                    &create_wasm_prologue(wranglerjs_output.wasm_name, self.get_wasm_binding());
            }
            None => {}
        }

        script += &wranglerjs_output.script;
        script_file.write_all(script.as_bytes())?;

        // cleanup {Webpack} dist.
        info!("Remove {}", wranglerjs_output.dist_to_clean);
        fs::remove_dir_all(wranglerjs_output.dist_to_clean).expect("could not clean Webpack dist.");

        // show output stats
        if self.has_wasm() {
            println!(
                "Sizes: wasm={} script={}",
                format_size(wranglerjs_output.wasm_size),
                format_size(wranglerjs_output.script_size)
            );
        } else {
            println!(
                "Sizes: script={}",
                format_size(wranglerjs_output.script_size)
            );
        }

        Ok(())
    }

    pub fn metadata_path(&self) -> String {
        Path::new(BUNDLE_OUT)
            .join("metadata.json".to_string())
            .to_str()
            .unwrap()
            .to_string()
    }

    pub fn wasm_path(&self) -> String {
        Path::new(BUNDLE_OUT)
            .join("module.wasm".to_string())
            .to_str()
            .unwrap()
            .to_string()
    }

    pub fn has_wasm(&self) -> bool {
        Path::new(&self.wasm_path()).exists()
    }

    pub fn has_webpack_config(&self) -> bool {
        Path::new("webpack.config.js").exists()
    }

    pub fn get_wasm_binding(&self) -> String {
        assert!(self.has_wasm());
        "wasmprogram".to_string()
    }

    pub fn script_path(&self) -> String {
        Path::new(BUNDLE_OUT)
            .join("script.js".to_string())
            .to_str()
            .unwrap()
            .to_string()
    }
}

// Path to {wrangler-js}, which should be executable.
fn executable_path() -> PathBuf {
    Path::new(".")
        .join("node_modules")
        .join(".bin")
        .join("wrangler-js")
}

// Run the underlying {wrangler-js} executable.
//
// In Rust we create a virtual file, pass the pass to {wrangler-js}, run the
// executable and wait for completion. The file will receive the a serialized
// {WrangerjsOutput} struct.
// Note that the ability to pass a fd is platform-specific
pub fn run_build(
    wasm_pack_path: PathBuf,
    bundle: &Bundle,
) -> Result<WrangerjsOutput, failure::Error> {
    if !Path::new(BUNDLE_OUT).exists() {
        fs::create_dir(BUNDLE_OUT)?;
    }

    let mut command = Command::new(executable_path());
    command.env("WASM_PACK_PATH", wasm_pack_path);

    // create temp file for special {wrangler-js} IPC.
    let mut temp_file = env::temp_dir();
    temp_file.push(".wranglerjs_output");
    File::create(temp_file.clone())?;

    command.arg(format!(
        "--output-file={}",
        temp_file.clone().to_str().unwrap().to_string()
    ));

    // if {webpack.config.js} is not present, we infer the entry based on the
    // {package.json} file and pass it to {wrangler-js}.
    // https://github.com/cloudflare/wrangler/issues/98
    if !bundle.has_webpack_config() {
        let package = Package::new("./")?;
        let current_dir = env::current_dir()?;
        let package_main = current_dir.join(package.main).to_str().unwrap().to_string();
        command.arg("--no-webpack-config=1");
        command.arg(format!("--use-entry={}", package_main));
    }

    info!("Running {:?}", command);

    let status = command.status()?;
    let output = fs::read_to_string(temp_file.clone()).expect("could not retrieve ouput");
    fs::remove_file(temp_file)?;

    if status.success() {
        Ok(serde_json::from_str(&output).expect("could not parse wranglerjs output"))
    } else {
        failure::bail!("failed to execute `{:?}`: exited with {}", command, status)
    }
}

pub fn run_npm_install() -> Result<(), failure::Error> {
    let mut command = Command::new("npm");
    command.arg("install");
    info!("Running {:?}", command);

    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        failure::bail!("failed to execute `{:?}`: exited with {}", command, status)
    }
}

// check if {wrangler-js} is present are a known location.
pub fn is_installed() -> bool {
    executable_path().exists()
}

pub fn install() -> Result<(), failure::Error> {
    let mut command = Command::new("npm");
    command.arg("install").arg("wrangler-js");
    info!("Running {:?}", command);

    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        failure::bail!("failed to execute `{:?}`: exited with {}", command, status)
    }
}

// We inject some code at the top-level of the Worker; called {prologue}.
// This aims to provide additional support, for instance providing {window}.
pub fn create_prologue() -> String {
    r#"
        const window = this;
    "#
    .to_string()
}

// Same idea as the {prologue} above, {Wasm} in {Webpack} requires to polyfill
// the {Fetch} function.
pub fn create_wasm_prologue(name: String, binding: String) -> String {
    format!(
        r#"
            const oldFetch = fetch;
            function fetch(name) {{
              if (name === "{name}") {{
                return Promise.resolve({{
                  arrayBuffer() {{
                    return {binding}; // defined in bindinds
                  }}
                }});
              }}
              return oldFetch(name);
            }}
        "#,
        name = name,
        binding = binding
    )
    .to_string()
}

// This metadata describe the bindings on the Worker.
fn create_metadata(bundle: &Bundle) -> String {
    info!("create metadata; wasm={}", bundle.has_wasm());
    if bundle.has_wasm() {
        format!(
            r#"
                {{
                    "body_part": "script",
                    "binding": {{
                        "name": "{name}",
                        "type": "wasm_module",
                        "part": "{name}"
                    }}
                }}
            "#,
            name = bundle.get_wasm_binding(),
        )
        .to_string()
    } else {
        format!(
            r#"
                {{
                    "body_part": "script"
                }}
            "#
        )
        .to_string()
    }
}

fn format_size(s: f32) -> String {
    match NumberPrefix::decimal(s) {
        Standalone(bytes) => format!("{} bytes", bytes),
        Prefixed(prefix, n) => format!("{:.0} {}B", n, prefix),
    }
}
