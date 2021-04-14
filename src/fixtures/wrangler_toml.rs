use std::collections::HashMap;

use serde::Serialize;

pub const TEST_ENV_NAME: &str = "test";

// small suite of flexible toml structs
// the  idea here is to focus on "when this config key is set"
// rather than needing to write tomls all the time.
// these structs set every value as an `Option`. To use,
// initialize a new WranglerToml::default() and begin setting
// values on it.
#[derive(Clone, Debug, Default, Serialize)]
pub struct KvConfig {
    pub binding: Option<&'static str>,
    pub id: Option<&'static str>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Triggers {
    pub crons: Option<Vec<String>>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SiteConfig {
    pub bucket: Option<&'static str>,
    #[serde(rename = "entry-point")]
    pub entry_point: Option<&'static str>,
    pub include: Option<Vec<&'static str>>,
    pub exclude: Option<Vec<&'static str>>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct EnvConfig {
    pub name: Option<&'static str>,
    pub account_id: Option<&'static str>,
    pub workers_dev: Option<bool>,
    pub route: Option<&'static str>,
    pub routes: Option<Vec<&'static str>>,
    pub zone_id: Option<&'static str>,
    pub webpack_config: Option<&'static str>,
    pub private: Option<bool>,
    pub site: Option<SiteConfig>,
    #[serde(alias = "kv-namespaces")]
    pub kv_namespaces: Option<Vec<KvConfig>>,
    pub vars: Option<HashMap<&'static str, &'static str>>,
    pub text_blobs: Option<HashMap<&'static str, &'static str>>,
    pub triggers: Option<Triggers>,
}

impl EnvConfig {
    pub fn custom_script_name(name: &'static str) -> EnvConfig {
        EnvConfig {
            name: Some(name),
            ..Default::default()
        }
    }

    pub fn zoneless(workers_dev: bool) -> EnvConfig {
        EnvConfig {
            workers_dev: Some(workers_dev),
            ..Default::default()
        }
    }

    pub fn zoneless_with_account_id(workers_dev: bool, account_id: &'static str) -> EnvConfig {
        EnvConfig {
            account_id: Some(account_id),
            workers_dev: Some(workers_dev),
            ..Default::default()
        }
    }

    pub fn zoned_single_route(zone_id: &'static str, route: &'static str) -> EnvConfig {
        EnvConfig {
            zone_id: Some(zone_id),
            route: Some(route),
            ..Default::default()
        }
    }

    pub fn zoned_multi_route(zone_id: &'static str, routes: Vec<&'static str>) -> EnvConfig {
        EnvConfig {
            zone_id: Some(zone_id),
            routes: Some(routes),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct WranglerToml {
    pub name: Option<&'static str>,
    #[serde(rename = "type")]
    pub target_type: Option<&'static str>,
    pub main: Option<&'static str>,
    pub account_id: Option<&'static str>,
    pub workers_dev: Option<bool>,
    pub route: Option<&'static str>,
    pub routes: Option<Vec<&'static str>>,
    pub zone_id: Option<&'static str>,
    pub webpack_config: Option<&'static str>,
    pub private: Option<bool>,
    pub env: Option<HashMap<&'static str, EnvConfig>>,
    #[serde(alias = "kv-namespaces")]
    pub kv_namespaces: Option<Vec<KvConfig>>,
    pub site: Option<SiteConfig>,
    pub vars: Option<HashMap<&'static str, &'static str>>,
    pub text_blobs: Option<HashMap<&'static str, &'static str>>,
    pub triggers: Option<Triggers>,
}

impl WranglerToml {
    // base build configs
    pub fn webpack(name: &'static str) -> WranglerToml {
        WranglerToml {
            name: Some(name),
            target_type: Some("webpack"),
            ..Default::default()
        }
    }

    pub fn zoneless(
        name: &'static str,
        account_id: &'static str,
        workers_dev: bool,
    ) -> WranglerToml {
        WranglerToml {
            workers_dev: Some(workers_dev),
            account_id: Some(account_id),
            ..WranglerToml::webpack(name)
        }
    }

    pub fn zoned_single_route(
        name: &'static str,
        zone_id: &'static str,
        route: &'static str,
    ) -> WranglerToml {
        let wrangler_toml = WranglerToml {
            zone_id: Some(zone_id),
            route: Some(route),
            ..WranglerToml::webpack(name)
        };
        eprintln!("{:#?}", &wrangler_toml);

        wrangler_toml
    }

    pub fn zoned_multi_route(
        name: &'static str,
        zone_id: &'static str,
        routes: Vec<&'static str>,
    ) -> WranglerToml {
        let wrangler_toml = WranglerToml {
            zone_id: Some(zone_id),
            routes: Some(routes),
            ..WranglerToml::webpack(name)
        };
        eprintln!("{:#?}", &wrangler_toml);

        wrangler_toml
    }

    pub fn with_env(name: &'static str, env_config: EnvConfig) -> WranglerToml {
        let wrangler_toml = WranglerToml {
            env: Some(test_env(env_config)),
            ..WranglerToml::webpack(name)
        };
        eprintln!("{:#?}", &wrangler_toml);

        wrangler_toml
    }

    pub fn zoneless_with_env(
        name: &'static str,
        account_id: &'static str,
        workers_dev: bool,
        env_config: EnvConfig,
    ) -> WranglerToml {
        let wrangler_toml = WranglerToml {
            env: Some(test_env(env_config)),
            ..WranglerToml::zoneless(name, account_id, workers_dev)
        };
        eprintln!("{:#?}", &wrangler_toml);

        wrangler_toml
    }

    pub fn zoned_single_route_with_env(
        name: &'static str,
        zone_id: &'static str,
        route: &'static str,
        env_config: EnvConfig,
    ) -> WranglerToml {
        let wrangler_toml = WranglerToml {
            env: Some(test_env(env_config)),
            ..WranglerToml::zoned_single_route(name, zone_id, route)
        };
        eprintln!("{:#?}", &wrangler_toml);

        wrangler_toml
    }

    pub fn webpack_build(name: &'static str) -> WranglerToml {
        WranglerToml {
            name: Some(name),
            workers_dev: Some(true),
            target_type: Some("webpack"),
            ..Default::default()
        }
    }

    pub fn webpack_std_config(name: &'static str) -> WranglerToml {
        WranglerToml {
            webpack_config: Some("webpack.config.js"),
            ..WranglerToml::webpack_build(name)
        }
    }

    pub fn webpack_custom_config(name: &'static str, webpack_config: &'static str) -> WranglerToml {
        WranglerToml {
            webpack_config: Some(webpack_config),
            ..WranglerToml::webpack_build(name)
        }
    }

    pub fn rust(name: &'static str) -> WranglerToml {
        WranglerToml {
            name: Some(name),
            workers_dev: Some(true),
            target_type: Some("rust"),
            ..Default::default()
        }
    }

    pub fn javascript(name: &'static str, main: &'static str) -> WranglerToml {
        WranglerToml {
            name: Some(name),
            workers_dev: Some(true),
            target_type: Some("javascript"),
            main: Some(main),
            ..Default::default()
        }
    }

    pub fn site(name: &'static str) -> WranglerToml {
        WranglerToml {
            site: Some(SiteConfig {
                bucket: Some("./public"),
                ..Default::default()
            }),
            ..WranglerToml::webpack_build(name)
        }
    }
}

fn test_env(env_config: EnvConfig) -> HashMap<&'static str, EnvConfig> {
    let mut env = HashMap::new();
    env.insert(TEST_ENV_NAME, env_config);

    env
}
