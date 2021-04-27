
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use crate::settings::get_wrangler_home_dir;
use std::fs::File;
use std::fs;
use std::io::Read;
use std::io::ErrorKind;
use failure::format_err;

static FILENAME: &str = "preinstalled_tools.json";

pub fn set_tool_path (tool: String, path: &Path) -> Result<(), failure::Error> {
    let mut tools = read()?;
    tools.insert(tool, path.to_str().unwrap().to_string());
    write(&tools)?;
    return Ok(());
}

pub fn get_tool_path (tool: String) -> Result<Option<PathBuf>, failure::Error> {
    return Ok(
        read()?
        .get(&tool)
        .map(|v| PathBuf::from(v))
    );
}

fn filename () -> Result<PathBuf, failure::Error> {
    return Ok(get_wrangler_home_dir()?.join(FILENAME));
}

fn read () -> Result<HashMap<String, String>, failure::Error> {
    let filename = filename()?;
    let f = File::open(&filename);
    let mut f = match f {
        Ok(file) => file,
        Err(error) => return match error.kind() {
            ErrorKind::NotFound => Ok(HashMap::new()),
            _ => Err(format_err!("error reading {}: {}", filename.to_str().unwrap(), error)),
        },
    };
    let mut contents = String::new();
    f.read_to_string(&mut contents)?;
    return Ok(serde_json::from_str(&contents)?);
}

fn write (tools: &HashMap<String, String>) -> Result<(), failure::Error> {
    fs::write(filename()?, serde_json::to_string(tools)?)?;
    return Ok(());
}
