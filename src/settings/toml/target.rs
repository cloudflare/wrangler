use super::kv_namespace::KvNamespace;
use super::site::Site;
use super::target_type::TargetType;
use super::UsageModel;

use std::collections::HashMap;
use std::env;

use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct Target {
    pub account_id: String,
    pub kv_namespaces: Vec<KvNamespace>,
    pub name: String,
    pub main: String,
    pub target_type: TargetType,
    pub webpack_config: Option<String>,
    pub site: Option<Site>,
    pub vars: Option<HashMap<String, String>>,
    pub text_blobs: Option<HashMap<String, PathBuf>>,
    pub usage_model: Option<UsageModel>,
}

impl Target {
    pub fn add_kv_namespace(&mut self, kv_namespace: KvNamespace) {
        self.kv_namespaces.push(kv_namespace);
    }

    pub fn build_dir(&self) -> Result<PathBuf, std::io::Error> {
        // if `site` is configured, we want to isolate worker code
        // and build artifacts away from static site application code.
        match &self.site {
            Some(site_config) => site_config.entry_point(),
            None => {
                let current_dir = env::current_dir()?;
                Ok(current_dir)
            }
        }
    }
}
