use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_with::rust::string_empty_as_none;

use crate::settings::toml::builder::Builder;
use crate::settings::toml::kv_namespace::ConfigKvNamespace;
use crate::settings::toml::route::RouteConfig;
use crate::settings::toml::site::Site;
use crate::settings::toml::triggers::Triggers;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Environment {
    pub name: Option<String>,
    #[serde(default, with = "string_empty_as_none")]
    pub account_id: Option<String>,
    pub workers_dev: Option<bool>,
    #[serde(default, with = "string_empty_as_none")]
    pub route: Option<String>,
    pub routes: Option<Vec<String>>,
    #[serde(default, with = "string_empty_as_none")]
    pub zone_id: Option<String>,
    pub webpack_config: Option<String>,
    pub build: Option<Builder>,
    pub private: Option<bool>,
    pub site: Option<Site>,
    #[serde(alias = "kv-namespaces")]
    pub kv_namespaces: Option<Vec<ConfigKvNamespace>>,
    pub vars: Option<HashMap<String, String>>,
    pub text_blobs: Option<HashMap<String, PathBuf>>,
    pub triggers: Option<Triggers>,
}

impl Environment {
    pub fn route_config(&self, top_level_route_config: &RouteConfig) -> Option<RouteConfig> {
        let account_id = self
            .account_id
            .clone()
            .or_else(|| top_level_route_config.account_id.clone());
        let zone_id = self
            .zone_id
            .clone()
            .or_else(|| top_level_route_config.zone_id.clone());
        let workers_dev = self.workers_dev.or(top_level_route_config.workers_dev);

        if self.workers_dev.is_none() && self.route.is_none() && self.routes.is_none() {
            None
        } else {
            Some(RouteConfig {
                account_id,
                workers_dev,
                route: self.route.clone(),
                routes: self.routes.clone(),
                zone_id,
            })
        }
    }
}
