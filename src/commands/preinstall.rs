use crate::wranglerjs::install;
use crate::settings::tools::set_tool_path;
use std::path::Path;

pub fn preinstall(tool: &str) -> Result<(), failure::Error> {
    return match tool {
        "wrangler-js" => install_wrangler_js(),
        _ => failure::bail!("this tool isn't supported for preinstallation")
    }
}

fn install_wrangler_js () -> Result<(), failure::Error> {
    let path = install()?;
    set_tool_path("wrangler-js".to_string(), Path::new(&path))?;
    return Ok(());
}