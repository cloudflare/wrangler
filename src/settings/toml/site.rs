use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::commands::generate::run_generate;

const SITE_ENTRY_POINT: &str = "workers-site";

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    pub bucket: PathBuf,
    #[serde(rename = "entry-point")]
    pub entry_point: Option<PathBuf>,
    pub include: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
}

impl Site {
    pub fn new(bucket: &str) -> Site {
        Site {
            bucket: PathBuf::from(bucket),
            ..Default::default()
        }
    }

    // if the user has configured `site.entry-point`, use that
    // as the build directory. Otherwise use the default const
    // SITE_ENTRY_POINT
    pub fn entry_point(&self) -> Result<PathBuf, std::io::Error> {
        let current_dir = env::current_dir()?;
        Ok(current_dir.join(
            self.entry_point
                .to_owned()
                .unwrap_or_else(|| PathBuf::from(SITE_ENTRY_POINT)),
        ))
    }

    pub fn scaffold_worker(&self) -> Result<(), failure::Error> {
        let entry_point = &self.entry_point()?;
        let template = "https://github.com/cloudflare/worker-sites-init";

        if !entry_point.exists() {
            log::info!("Generating a new workers site project");
            run_generate(
                entry_point.file_name().unwrap().to_str().unwrap(),
                template,
                None,
            )?;

            // This step is to prevent having a git repo within a git repo after
            // generating the scaffold into an existing project.
            fs::remove_dir_all(&entry_point.join(".git"))?;
        }

        Ok(())
    }
}

impl Default for Site {
    fn default() -> Site {
        Site {
            bucket: PathBuf::new(),
            entry_point: Some(PathBuf::from(SITE_ENTRY_POINT)),
            include: None,
            exclude: None,
        }
    }
}
