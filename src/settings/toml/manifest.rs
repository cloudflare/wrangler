use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use config::{Config, File};

use serde::{Deserialize, Serialize};
use serde_with::rust::string_empty_as_none;

use super::UsageModel;
use crate::commands::{validate_worker_name, whoami, DEFAULT_CONFIG_PATH};
use crate::deploy::{self, DeployTarget, DeploymentSet};
use crate::settings::toml::builder::Builder;
use crate::settings::toml::dev::Dev;
use crate::settings::toml::environment::Environment;
use crate::settings::toml::kv_namespace::{ConfigKvNamespace, KvNamespace};
use crate::settings::toml::route::RouteConfig;
use crate::settings::toml::site::Site;
use crate::settings::toml::target_type::TargetType;
use crate::settings::toml::triggers::Triggers;
use crate::settings::toml::Target;
use crate::terminal::{
    emoji,
    message::{Message, StdOut},
    styles,
};

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Manifest {
    #[serde(default)]
    pub name: String,
    #[serde(rename = "type")]
    pub target_type: TargetType,
    #[serde(default)]
    pub account_id: String,
    pub workers_dev: Option<bool>,
    #[serde(default, with = "string_empty_as_none")]
    pub route: Option<String>,
    pub routes: Option<Vec<String>>,
    #[serde(default, with = "string_empty_as_none")]
    pub zone_id: Option<String>,
    pub webpack_config: Option<String>,
    pub build: Option<Builder>,
    pub private: Option<bool>,
    // TODO: maybe one day, serde toml support will allow us to serialize sites
    // as a TOML inline table (this would prevent confusion with environments too!)
    pub site: Option<Site>,
    pub dev: Option<Dev>,
    #[serde(alias = "kv-namespaces")]
    pub kv_namespaces: Option<Vec<ConfigKvNamespace>>,
    pub env: Option<HashMap<String, Environment>>,
    pub vars: Option<HashMap<String, String>>,
    pub text_blobs: Option<HashMap<String, PathBuf>>,
    pub wasm_modules: Option<HashMap<String, PathBuf>>,
    pub triggers: Option<Triggers>,
    #[serde(default, with = "string_empty_as_none")]
    pub usage_model: Option<UsageModel>,
}

impl Manifest {
    pub fn new(config_path: &Path) -> Result<Self, failure::Error> {
        let file_name = config_path.file_name().unwrap().to_str().unwrap();
        let mut message = format!("{} not found", file_name);
        if config_path.to_str().unwrap() == DEFAULT_CONFIG_PATH {
            message.push_str("; run `wrangler init` to create one.");
        }
        failure::ensure!(config_path.exists(), message);
        let config = read_config(config_path)?;

        let manifest: Manifest = match config.try_into() {
            Ok(m) => m,
            Err(e) => {
                if e.to_string().contains("unknown field `kv-namespaces`") {
                    failure::bail!("kv-namespaces should not live under the [site] table in your configuration file; please move it above [site].")
                } else {
                    failure::bail!(e)
                }
            }
        };

        check_for_duplicate_names(&manifest)?;

        Ok(manifest)
    }

    pub fn generate(
        name: String,
        target_type: Option<TargetType>,
        config_path: &PathBuf,
        site: Option<Site>,
    ) -> Result<Manifest, failure::Error> {
        let config_file = &config_path.join("wrangler.toml");
        let config_template_str = fs::read_to_string(config_file).unwrap_or_else(|err| {
            log::info!("Error reading config template: {}", err);
            log::info!("Using default instead");
            toml::to_string_pretty(&Manifest::default())
                .expect("serializing the default toml should never fail")
        });

        let config_template =
            toml::from_str::<Manifest>(&config_template_str).unwrap_or_else(|err| {
                log::info!("Error parsing config template: {}", err);
                log::info!("Using default instead");
                Manifest::default()
            });

        config_template.warn_on_account_info();
        if let Some(target_type) = &target_type {
            if config_template.target_type != *target_type {
                StdOut::warn(&format!("The template recommends the \"{}\" type. Using type \"{}\" may cause errors, we recommend changing the type field in wrangler.toml to \"{}\"", config_template.target_type, target_type, config_template.target_type));
            }
        }

        let default_workers_dev = match &config_template.route {
            Some(route) if route.is_empty() => Some(true),
            None => Some(true),
            _ => None,
        };

        /*
         * We use toml-edit for actually changing the template provided wrangler.toml,
         * since toml-edit is a format-preserving parser. Elsewhere, we use only toml-rs,
         * as only toml-rs supports serde.
         */

        let mut config_template_doc =
            config_template_str
                .parse::<toml_edit::Document>()
                .map_err(|err| {
                    failure::err_msg(format!(
                        "toml_edit failed to parse config template. {}",
                        err
                    ))
                })?;

        config_template_doc["name"] = toml_edit::value(name);
        if let Some(default_workers_dev) = default_workers_dev {
            config_template_doc["workers_dev"] = toml_edit::value(default_workers_dev);
        }
        if let Some(target_type) = &target_type {
            config_template_doc["target_type"] = toml_edit::value(target_type.to_string());
        }
        if let Some(site) = site {
            if config_template.site.is_none() {
                config_template_doc["site"]["bucket"] =
                    toml_edit::value(site.bucket.to_string_lossy().as_ref());

                if let Some(entry_point) = &site.entry_point {
                    config_template_doc["site"]["entry-point"] =
                        toml_edit::value(entry_point.to_string_lossy().as_ref());
                }
                if let Some(include) = &site.include {
                    let mut arr = toml_edit::Array::default();
                    include.iter().for_each(|i| {
                        arr.push(i.as_ref()).unwrap();
                    });
                    config_template_doc["site"]["include"] = toml_edit::value(arr);
                }
                if let Some(exclude) = &site.exclude {
                    let mut arr = toml_edit::Array::default();
                    exclude.iter().for_each(|i| {
                        arr.push(i.as_ref()).unwrap();
                    });
                    config_template_doc["site"]["exclude"] = toml_edit::value(arr);
                }
            }
        }

        // TODO: https://github.com/cloudflare/wrangler/issues/773

        let toml = config_template_doc.to_string_in_original_order();
        let manifest = toml::from_str::<Manifest>(&toml)?;

        log::info!("Writing a wrangler.toml file at {}", config_file.display());
        fs::write(&config_file, &toml)?;
        Ok(manifest)
    }

    pub fn worker_name(&self, env_arg: Option<&str>) -> String {
        if let Some(environment) = self.get_environment(env_arg).unwrap_or_default() {
            if let Some(name) = &environment.name {
                return name.clone();
            }
            if let Some(env) = env_arg {
                return format!("{}-{}", self.name, env);
            }
        }

        self.name.clone()
    }

    fn route_config(&self) -> RouteConfig {
        RouteConfig {
            account_id: Some(self.account_id.clone()),
            workers_dev: self.workers_dev,
            route: self.route.clone(),
            routes: self.routes.clone(),
            zone_id: self.zone_id.clone(),
        }
    }

    pub fn get_deployments(&self, env: Option<&str>) -> Result<DeploymentSet, failure::Error> {
        let script = self.worker_name(env);
        validate_worker_name(&script)?;

        let mut deployments = DeploymentSet::new();

        let env = self.get_environment(env)?;

        let mut add_routed_deployments =
            |route_config: &RouteConfig| -> Result<(), failure::Error> {
                if route_config.is_zoned() {
                    let zoned = deploy::ZonedTarget::build(&script, route_config)?;
                    // This checks all of the configured routes for the wildcard ending and warns
                    // the user that their site may not work as expected without it.
                    if self.site.is_some() {
                        let no_star_routes = zoned
                            .routes
                            .iter()
                            .filter(|r| !r.pattern.ends_with('*'))
                            .map(|r| r.pattern.as_str())
                            .collect::<Vec<_>>();
                        if !no_star_routes.is_empty() {
                            StdOut::warn(&format!(
                            "The following routes in your configuration file should have a trailing * to apply the Worker on every path, otherwise your site will not behave as expected.\n{}",
                            no_star_routes.join("\n"))
                        );
                        }
                    }

                    deployments.push(DeployTarget::Zoned(zoned));
                }

                if route_config.is_zoneless() {
                    let zoneless = deploy::ZonelessTarget::build(&script, route_config)?;
                    deployments.push(DeployTarget::Zoneless(zoneless));
                }

                Ok(())
            };

        let top_level_route_config = self.route_config();

        if let Some(env) = env {
            if let Some(env_route_cfg) = env.route_config(&top_level_route_config) {
                add_routed_deployments(&env_route_cfg)
            } else if top_level_route_config.is_zoned() {
                failure::bail!("you must specify route(s) per environment for zoned deploys.");
            } else {
                add_routed_deployments(&top_level_route_config)
            }
        } else {
            add_routed_deployments(&top_level_route_config)
        }?;

        let crons = match env {
            Some(e) => {
                let account_id = e.account_id.as_ref().unwrap_or(&self.account_id);
                e.triggers
                    .as_ref()
                    .or_else(|| self.triggers.as_ref())
                    .map(|t| (t.crons.as_slice(), account_id))
            }
            None => self
                .triggers
                .as_ref()
                .map(|t| (t.crons.as_slice(), &self.account_id)),
        };

        if let Some((crons, account)) = crons {
            let scheduled =
                deploy::ScheduleTarget::build(account.clone(), script.clone(), crons.to_vec())?;
            deployments.push(DeployTarget::Schedule(scheduled));
        }

        if deployments.is_empty() {
            failure::bail!("No deployments specified!")
        }

        Ok(deployments)
    }

    pub fn get_account_id(&self, environment_name: Option<&str>) -> Result<String, failure::Error> {
        let environment = self.get_environment(environment_name)?;
        let mut result = self.account_id.to_string();
        if let Some(environment) = environment {
            if let Some(account_id) = &environment.account_id {
                result = account_id.to_string();
            }
        }
        if result.is_empty() {
            let mut msg = "Your configuration file is missing an account_id field".to_string();
            if let Some(environment_name) = environment_name {
                msg.push_str(&format!(" in [env.{}]", environment_name));
            }
            failure::bail!("{}", &msg)
        } else {
            Ok(result)
        }
    }

    pub fn get_target(
        &self,
        environment_name: Option<&str>,
        preview: bool,
    ) -> Result<Target, failure::Error> {
        if self.site.is_some() {
            match self.target_type {
                TargetType::Rust => {
                    failure::bail!(format!(
                        "{} Workers Sites does not support Rust type projects.",
                        emoji::WARN
                    ))
                }
                TargetType::JavaScript => {
                    let error_message = format!(
                        "{} Workers Sites requires using a bundler, and your configuration indicates that you aren't using one. You can fix this by:\n* setting your project type to \"webpack\" to use our automatically configured webpack bundler.\n* setting your project type to \"javascript\", and configuring a build command in the `[build]` section if you wish to use your choice of bundler.",
                        emoji::WARN
                    );
                    if let Some(build) = &self.build {
                        if build.command.is_none() {
                            failure::bail!(error_message)
                        }
                    } else {
                        failure::bail!(error_message)
                    }
                }
                _ => {}
            }
        }

        /*
        From https://developers.cloudflare.com/workers/cli-wrangler/configuration#keys
        Top level: required to be configured at the top level of your wrangler.toml only; multiple environments on the same project must share this property

        Inherited: Can be configured at the top level and/or environment. If the property is defined only at the top level, the environment will use the property value from the top level. If the property is defined in the environment, the environment value will override the top level value.

        Not inherited: Must be defined for every environment individually.
        */
        let mut target = Target {
            target_type: self.target_type.clone(),       // Top level
            account_id: self.account_id.clone(),         // Inherited
            webpack_config: self.webpack_config.clone(), // Inherited
            build: self.build.clone(),                   // Inherited
            // importantly, the top level name will be modified
            // to include the name of the environment
            name: self.name.clone(), // Inherited
            kv_namespaces: get_namespaces(self.kv_namespaces.clone(), preview)?, // Not inherited
            site: self.site.clone(), // Inherited
            vars: self.vars.clone(), // Not inherited
            text_blobs: self.text_blobs.clone(), // Inherited
            usage_model: self.usage_model, // Top level
            wasm_modules: self.wasm_modules.clone(),
        };

        let environment = self.get_environment(environment_name)?;

        if let Some(environment) = environment {
            target.name = self.worker_name(environment_name);
            if let Some(account_id) = &environment.account_id {
                target.account_id = account_id.clone();
            }
            if let Some(webpack_config) = &environment.webpack_config {
                target.webpack_config = Some(webpack_config.clone());
            }
            if let Some(build) = &environment.build {
                target.build = Some(build.clone());
            }

            // don't inherit kv namespaces because it is an anti-pattern to use the same namespaces across multiple environments
            target.kv_namespaces = get_namespaces(environment.kv_namespaces.clone(), preview)?;

            // inherit site configuration
            if let Some(site) = &environment.site {
                target.site = Some(site.clone());
            }

            // don't inherit vars
            target.vars = environment.vars.clone();
        }

        Ok(target)
    }

    pub fn get_environment(
        &self,
        environment_name: Option<&str>,
    ) -> Result<Option<&Environment>, failure::Error> {
        // check for user-specified environment name
        if let Some(environment_name) = environment_name {
            if let Some(environment_table) = &self.env {
                if let Some(environment) = environment_table.get(environment_name) {
                    Ok(Some(environment))
                } else {
                    failure::bail!(format!(
                        "{} Could not find environment with name \"{}\"",
                        emoji::WARN,
                        environment_name
                    ))
                }
            } else {
                failure::bail!(format!(
                    "{} There are no environments specified in your configuration file",
                    emoji::WARN
                ))
            }
        } else {
            Ok(None)
        }
    }

    fn warn_on_account_info(&self) {
        let account_id_env = env::var("CF_ACCOUNT_ID").is_ok();
        let zone_id_env = env::var("CF_ZONE_ID").is_ok();
        let mut top_level_fields: Vec<String> = Vec::new();
        if !account_id_env {
            top_level_fields.push("account_id".to_string());
        }
        if let Some(kv_namespaces) = &self.kv_namespaces {
            for kv_namespace in kv_namespaces {
                top_level_fields.push(format!(
                    "kv-namespace {} needs a namespace_id",
                    kv_namespace.binding
                ));
            }
        }
        if let Some(route) = &self.route {
            if !route.is_empty() {
                top_level_fields.push("route".to_string());
            }
        }
        if let Some(zone_id) = &self.zone_id {
            if !zone_id.is_empty() && !zone_id_env {
                top_level_fields.push("zone_id".to_string());
            }
        }

        let mut env_fields: HashMap<String, Vec<String>> = HashMap::new();

        if let Some(env) = &self.env {
            for (env_name, env) in env {
                let mut current_env_fields: Vec<String> = Vec::new();
                if env.account_id.is_some() && !account_id_env {
                    current_env_fields.push("account_id".to_string());
                }
                if let Some(kv_namespaces) = &env.kv_namespaces {
                    for kv_namespace in kv_namespaces {
                        current_env_fields.push(format!(
                            "kv-namespace {} needs a namespace_id",
                            kv_namespace.binding
                        ));
                    }
                }
                if let Some(route) = &env.route {
                    if !route.is_empty() {
                        current_env_fields.push("route".to_string());
                    }
                }
                if let Some(zone_id) = &env.zone_id {
                    if !zone_id.is_empty() && !zone_id_env {
                        current_env_fields.push("zone_id".to_string());
                    }
                }
                if !current_env_fields.is_empty() {
                    env_fields.insert(env_name.to_string(), current_env_fields);
                }
            }
        }
        let has_top_level_fields = !top_level_fields.is_empty();
        let has_env_fields = !env_fields.is_empty();
        let mut needs_new_line = false;
        if has_top_level_fields || has_env_fields {
            let toml_msg = styles::highlight("wrangler.toml");
            let zone_id_msg = styles::highlight("zone_id");
            let dash_url = styles::url("https://dash.cloudflare.com");

            StdOut::help(&format!(
                "You can find your {} in the right sidebar of a zone's overview tab at {}",
                zone_id_msg, dash_url
            ));

            whoami::display_account_id_maybe();

            StdOut::help(
                &format!("You will need to update the following fields in the created {} file before continuing:", toml_msg)
            );

            if has_top_level_fields {
                needs_new_line = true;
                for top_level_field in top_level_fields {
                    println!("- {}", top_level_field);
                }
            }

            if has_env_fields {
                for (env_name, env_fields) in env_fields {
                    if needs_new_line {
                        println!();
                    }
                    println!("[env.{}]", env_name);
                    needs_new_line = true;
                    for env_field in env_fields {
                        println!("  - {}", env_field);
                    }
                }
            }
        }
    }
}

impl FromStr for Manifest {
    type Err = toml::de::Error;

    fn from_str(serialized_toml: &str) -> Result<Self, Self::Err> {
        toml::from_str(serialized_toml)
    }
}

fn read_config(config_path: &Path) -> Result<Config, failure::Error> {
    let mut config = Config::new();

    let config_str = config_path
        .to_str()
        .expect("project config path should be a string");
    config.merge(File::with_name(config_str))?;

    // Eg.. `CF_ACCOUNT_AUTH_KEY=farts` would set the `account_auth_key` key
    config.merge(config::Environment::with_prefix("CF"))?;

    Ok(config)
}

fn check_for_duplicate_names(manifest: &Manifest) -> Result<(), failure::Error> {
    let mut names: HashSet<String> = HashSet::new();
    let mut duplicate_names: HashSet<String> = HashSet::new();
    names.insert(manifest.name.to_string());
    if let Some(environments) = &manifest.env {
        for (_, environment) in environments.iter() {
            if let Some(name) = &environment.name {
                if names.contains(name) && !duplicate_names.contains(name) {
                    duplicate_names.insert(name.to_string());
                } else {
                    names.insert(name.to_string());
                }
            }
        }
    }
    let duplicate_name_string = duplicate_names
        .clone()
        .into_iter()
        .collect::<Vec<String>>()
        .join(", ");
    let duplicate_message = match duplicate_names.len() {
        1 => Some("this name is duplicated".to_string()),
        n if n >= 2 => Some("these names are duplicated".to_string()),
        _ => None,
    };
    if let Some(message) = duplicate_message {
        failure::bail!(format!(
            "{} Each name in your configuration file must be unique, {}: {}",
            emoji::WARN,
            message,
            duplicate_name_string
        ))
    }
    Ok(())
}

fn get_namespaces(
    kv_namespaces: Option<Vec<ConfigKvNamespace>>,
    preview: bool,
) -> Result<Vec<KvNamespace>, failure::Error> {
    if let Some(namespaces) = kv_namespaces {
        namespaces.into_iter().map(|ns| {
            if preview {
                if let Some(preview_id) = &ns.preview_id {
                    if let Some(id) = &ns.id {
                        if preview_id == id {
                            StdOut::warn("Specifying the same KV namespace ID for both preview and production sessions may cause bugs in your production worker! Proceed with caution.");
                        }
                    }
                    Ok(KvNamespace {
                        id: preview_id.to_string(),
                        binding: ns.binding.to_string(),
                    })
                } else {
                    failure::bail!("In order to preview a worker with KV namespaces, you must designate a preview_id in your configuration file for each KV namespace you'd like to preview.")
                }
            } else if let Some(id) = &ns.id {
                Ok(KvNamespace {
                    id: id.to_string(),
                    binding: ns.binding,
                })
            } else {
                failure::bail!("You must specify the namespace ID in the id field for the namespace with binding \"{}\"", &ns.binding)
            }
        }).collect()
    } else {
        Ok(Vec::new())
    }
}
