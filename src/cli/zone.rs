use crate::commands;
use crate::settings::global_user::GlobalUser;

use anyhow::Result;

pub fn zone(zone: String) -> Result<()> {
    log::info!("Getting User settings");
    commands::zone(&GlobalUser::new()?, zone)
}
