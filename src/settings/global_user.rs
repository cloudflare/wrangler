use std::fs;
use std::path::{Path, PathBuf};

use cloudflare::framework::auth::Credentials;
use serde::{Deserialize, Serialize};

use crate::settings::http_config::HttpConfig;
use crate::settings::{get_global_config_path, Environment, QueryEnvironment};
use crate::terminal::{emoji, styles};

const CF_API_TOKEN: &str = "CF_API_TOKEN";
const CF_API_KEY: &str = "CF_API_KEY";
const CF_EMAIL: &str = "CF_EMAIL";
const CF_CONNECT_TIMEOUT: &str = "CF_CONNECT_TIMEOUT";
const CF_HTTP_TIMEOUT: &str = "CF_HTTP_TIMEOUT";
const CF_BULK_TIMEOUT: &str = "CF_BULK_TIMEOUT";

static ENV_VAR_WHITELIST: [&str; 6] = [
    CF_API_TOKEN,
    CF_API_KEY,
    CF_EMAIL,
    CF_CONNECT_TIMEOUT,
    CF_HTTP_TIMEOUT,
    CF_BULK_TIMEOUT,
];

static NON_CF_XXX_TIMEOUT_ENV_VARS: [&str; 3] = [CF_API_TOKEN, CF_API_KEY, CF_EMAIL];

#[cfg(test)]
use std::io::Write;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(untagged)]
pub enum GlobalUser {
    TokenAuth {
        api_token: String,
        #[serde(flatten)]
        http_config: HttpConfig,
    },
    GlobalKeyAuth {
        email: String,
        api_key: String,
        #[serde(flatten)]
        http_config: HttpConfig,
    },
}

impl GlobalUser {
    pub fn new() -> Result<Self, failure::Error> {
        let environment = Environment::with_whitelist(ENV_VAR_WHITELIST.to_vec());

        let config_path = get_global_config_path()?;
        GlobalUser::build(environment, config_path)
    }

    pub fn get_http_config(&self) -> &HttpConfig {
        match &self {
            GlobalUser::TokenAuth { http_config, .. } => http_config,
            GlobalUser::GlobalKeyAuth { http_config, .. } => http_config,
        }
    }

    fn build<T: 'static + QueryEnvironment>(
        environment: T,
        config_path: PathBuf,
    ) -> Result<Self, failure::Error>
    where
        T: config::Source + Send + Sync,
    {
        if let Some(user) = Self::from_env(environment, config_path.clone()) {
            user
        } else {
            Self::from_file(config_path)
        }
    }

    fn from_env<T: 'static + QueryEnvironment>(
        environment: T,
        config_path: PathBuf,
    ) -> Option<Result<Self, failure::Error>>
    where
        T: config::Source + Send + Sync,
    {
        // if there's some problem with gathering the environment,
        // or if there are no relevant environment variables set,
        // fall back to config file.
        if environment.empty().unwrap_or(true) {
            None
        } else {
            let mut s = config::Config::new();

            // attempt to merge the on disk configuration with the environment
            // doing this allows users to setup a configuration file with their
            // api keys, and additionally use environment variables to override
            // timeouts
            //
            // the disk configuration is merged first so that the environment
            // configuration (merged later) will take priority
            // see: https://github.com/mehcode/config-rs/blob/fb33478fe6863712a699c258c533a53340d5611f/examples/hierarchical-env/src/settings.rs#L43-L50
            //
            // for example:
            //
            // ```toml
            // api_token = "secret_token_here"
            // ```
            //
            // $ CF_HTTP_TIMEOUT=600 wrangler publish
            //
            // to support backwards compatibility, this merge is **only** done
            // if the environment contains only CF_XXX_TIMEOUT variables.
            // otherwise, we would break test `it_can_prioritize_env_vars`
            let config_str = config_path
                .to_str()
                .expect("global config path should be a string");

            if config_path.exists() && has_only_cf_xxx_timeout_vars(&environment) {
                log::info!(
                    "Config path exists. Reading from config file, {}",
                    config_str
                );
                s.merge(config::File::with_name(config_str)).ok();
            }

            s.merge(environment).ok();

            Some(GlobalUser::from_config(s))
        }
    }

    fn from_file(config_path: PathBuf) -> Result<Self, failure::Error> {
        let mut s = config::Config::new();

        let config_str = config_path
            .to_str()
            .expect("global config path should be a string");

        // Skip reading global config if non existent
        // because envs might be provided
        if config_path.exists() {
            log::info!(
                "Config path exists. Reading from config file, {}",
                config_str
            );
            s.merge(config::File::with_name(config_str))?;
        } else {
            failure::bail!(
                "config path does not exist {}. Try running `wrangler login` or `wrangler config`",
                config_str
            );
        }

        GlobalUser::from_config(s)
    }

    pub fn to_file(&self, config_path: &Path) -> Result<(), failure::Error> {
        let toml = toml::to_string(self)?;

        fs::create_dir_all(&config_path.parent().unwrap())?;
        fs::write(&config_path, toml)?;

        Ok(())
    }

    fn from_config(config: config::Config) -> Result<Self, failure::Error> {
        let global_user: Result<GlobalUser, config::ConfigError> = config.clone().try_into();
        match global_user {
            Ok(user) => Ok(user),
            Err(_) => {
                let wrangler_login_msg = styles::highlight("`wrangler login`");
                let wrangler_config_msg = styles::highlight("`wrangler config`");
                let vars_msg = styles::url("https://developers.cloudflare.com/workers/tooling/wrangler/configuration/#using-environment-variables");
                let msg = format!(
                    "{} Your authentication details are improperly configured.\nPlease run {}, {}, or visit\n{}\nfor info on configuring with environment variables",
                    emoji::WARN,
                    wrangler_login_msg,
                    wrangler_config_msg,
                    vars_msg
                );
                log::info!("{:?}", config);
                failure::bail!(msg)
            }
        }
    }
}

impl From<GlobalUser> for Credentials {
    fn from(user: GlobalUser) -> Credentials {
        match user {
            GlobalUser::TokenAuth { api_token, .. } => {
                Credentials::UserAuthToken { token: api_token }
            }
            GlobalUser::GlobalKeyAuth { email, api_key, .. } => Credentials::UserAuthKey {
                key: api_key,
                email,
            },
        }
    }
}

fn has_only_cf_xxx_timeout_vars(env: &impl QueryEnvironment) -> bool {
    for &timeout_var in NON_CF_XXX_TIMEOUT_ENV_VARS.iter() {
        let has_non_timeout_var = env
            .get_var(timeout_var)
            .map(|s| !s.is_empty())
            .unwrap_or(false);

        if has_non_timeout_var {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, time::Duration};
    use tempfile::tempdir;

    use crate::settings::{environment::MockEnvironment, DEFAULT_CONFIG_FILE_NAME};

    #[test]
    fn it_can_prioritize_token_input() {
        // Set all CF_API_TOKEN, CF_EMAIL, and CF_API_KEY.
        // This test evaluates whether the GlobalUser returned is
        // a GlobalUser::TokenAuth (expected behavior; token
        // should be prioritized over email + global API key pair.)
        let mut mock_env = MockEnvironment::default();
        mock_env.set(CF_API_TOKEN, "foo");
        mock_env.set(CF_EMAIL, "test@cloudflare.com");
        mock_env.set(CF_API_KEY, "bar");

        let tmp_dir = tempdir().unwrap();
        let config_dir = test_config_dir(&tmp_dir, None).unwrap();

        let user = GlobalUser::build(mock_env, config_dir).unwrap();
        assert_eq!(
            user,
            GlobalUser::TokenAuth {
                api_token: "foo".to_string(),
                http_config: Default::default(),
            }
        );
    }

    #[test]
    fn it_can_prioritize_env_vars() {
        let api_token = "thisisanapitoken";
        let api_key = "reallylongglobalapikey";
        let email = "user@example.com";

        let file_user = GlobalUser::TokenAuth {
            api_token: api_token.to_string(),
            http_config: Default::default(),
        };
        let env_user = GlobalUser::GlobalKeyAuth {
            api_key: api_key.to_string(),
            email: email.to_string(),
            http_config: Default::default(),
        };

        let mut mock_env = MockEnvironment::default();
        mock_env.set(CF_EMAIL, email);
        mock_env.set(CF_API_KEY, api_key);

        let tmp_dir = tempdir().unwrap();
        let tmp_config_path = test_config_dir(&tmp_dir, Some(file_user)).unwrap();

        let new_user = GlobalUser::build(mock_env, tmp_config_path).unwrap();

        assert_eq!(new_user, env_user);
    }

    #[test]
    fn it_falls_through_to_config_with_no_env_vars() {
        let mock_env = MockEnvironment::default();

        let user = GlobalUser::TokenAuth {
            api_token: "thisisanapitoken".to_string(),
            http_config: Default::default(),
        };

        let tmp_dir = tempdir().unwrap();
        let tmp_config_path = test_config_dir(&tmp_dir, Some(user.clone())).unwrap();

        let new_user = GlobalUser::build(mock_env, tmp_config_path).unwrap();

        assert_eq!(new_user, user);
    }

    #[test]
    fn it_fails_if_global_auth_incomplete_in_file() {
        let tmp_dir = tempdir().unwrap();
        let config_dir = test_config_dir(&tmp_dir, None).unwrap();

        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&config_dir.as_path())
            .unwrap();
        let email_config = "email = \"thisisanemail\"";
        file.write_all(email_config.as_bytes()).unwrap();

        let file_user = GlobalUser::from_file(config_dir);

        assert!(file_user.is_err());
    }

    #[test]
    fn it_fails_if_global_auth_incomplete_in_env() {
        let mut mock_env = MockEnvironment::default();

        mock_env.set(CF_API_KEY, "apikey");

        let tmp_dir = tempdir().unwrap();
        let config_dir = test_config_dir(&tmp_dir, None).unwrap();

        let new_user = GlobalUser::build(mock_env, config_dir);

        assert!(new_user.is_err());
    }

    #[test]
    fn it_succeeds_with_no_config() {
        let mut mock_env = MockEnvironment::default();
        mock_env.set(CF_API_KEY, "apikey");
        mock_env.set(CF_EMAIL, "email");
        let dummy_path = Path::new("./definitely-does-not-exist.txt").to_path_buf();
        let new_user = GlobalUser::build(mock_env, dummy_path);

        assert!(new_user.is_ok());
    }

    #[test]
    fn token_auth_succeeds_with_config_timeouts() {
        let tmp_dir = tempdir().unwrap();
        let config_dir = test_config_dir(&tmp_dir, None).unwrap();

        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&config_dir.as_path())
            .unwrap();
        let config = r#"
api_token = "my_api_token"
connect_timeout = 3
bulk_timeout = 16
"#;
        file.write_all(config.as_bytes()).unwrap();

        let new_user = GlobalUser::build(MockEnvironment::default(), config_dir).unwrap();
        let http_config = match new_user {
            GlobalUser::TokenAuth { http_config, .. } => http_config,
            _ => panic!("expected TokenAuth user"),
        };

        assert_eq!(Duration::from_secs(3), http_config.get_connect_timeout());
        assert_eq!(
            Duration::from_secs(crate::http::DEFAULT_HTTP_TIMEOUT_SECONDS),
            http_config.get_http_timeout()
        );
        assert_eq!(Duration::from_secs(16), http_config.get_bulk_timeout());
    }

    #[test]
    fn global_auth_succeeds_with_config_timeouts() {
        let tmp_dir = tempdir().unwrap();
        let config_dir = test_config_dir(&tmp_dir, None).unwrap();

        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&config_dir.as_path())
            .unwrap();
        let config = r#"
email = "workers@cloudflare.com"
api_key = "my_api_key"
http_timeout = 3
"#;
        file.write_all(config.as_bytes()).unwrap();

        let new_user = GlobalUser::build(MockEnvironment::default(), config_dir).unwrap();
        let http_config = match new_user {
            GlobalUser::GlobalKeyAuth { http_config, .. } => http_config,
            _ => panic!("expected TokenAuth user"),
        };

        assert_eq!(
            Duration::from_secs(crate::http::DEFAULT_CONNECT_TIMEOUT_SECONDS),
            http_config.get_connect_timeout()
        );
        assert_eq!(Duration::from_secs(3), http_config.get_http_timeout());
        assert_eq!(
            Duration::from_secs(crate::http::DEFAULT_BULK_TIMEOUT_SECONDS),
            http_config.get_bulk_timeout()
        );
    }

    #[test]
    fn auth_succeeds_with_env_str_and_num_timeouts() {
        let tmp_dir = tempdir().unwrap();
        let config_dir = test_config_dir(&tmp_dir, None).unwrap();

        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&config_dir.as_path())
            .unwrap();
        let config = r#"
email = "workers@cloudflare.com"
api_key = "my_api_key"
connect_timeout = 8
http_timeout = 3
"#;
        file.write_all(config.as_bytes()).unwrap();

        let new_user = GlobalUser::build(MockEnvironment::default(), config_dir).unwrap();
        let http_config = match new_user {
            GlobalUser::GlobalKeyAuth { http_config, .. } => http_config,
            _ => panic!("expected TokenAuth user"),
        };

        assert_eq!(Duration::from_secs(8), http_config.get_connect_timeout());
        assert_eq!(Duration::from_secs(3), http_config.get_http_timeout());
        assert_eq!(
            Duration::from_secs(crate::http::DEFAULT_BULK_TIMEOUT_SECONDS),
            http_config.get_bulk_timeout()
        );
    }

    #[test]
    fn environment_timeouts_get_applied_to_config() {
        let tmp_dir = tempdir().unwrap();
        let config_dir = test_config_dir(&tmp_dir, None).unwrap();

        let mut file = fs::OpenOptions::new()
            .write(true)
            .open(&config_dir.as_path())
            .unwrap();
        let config = r#"
email = "workers@cloudflare.com"
api_key = "my_api_key"
"#;
        file.write_all(config.as_bytes()).unwrap();

        let mut env = MockEnvironment::default();
        env.set(CF_HTTP_TIMEOUT, "10000");
        env.set(CF_BULK_TIMEOUT, "10000");

        let new_user = GlobalUser::build(env, config_dir).unwrap();
        let http_config = match new_user {
            GlobalUser::GlobalKeyAuth { http_config, .. } => http_config,
            _ => panic!("expected TokenAuth user"),
        };

        assert_eq!(
            Duration::from_secs(crate::http::DEFAULT_CONNECT_TIMEOUT_SECONDS),
            http_config.get_connect_timeout()
        );
        assert_eq!(Duration::from_secs(10_000), http_config.get_http_timeout());
        assert_eq!(Duration::from_secs(10_000), http_config.get_bulk_timeout());
    }

    fn test_config_dir(
        tmp_dir: &tempfile::TempDir,
        user: Option<GlobalUser>,
    ) -> Result<PathBuf, failure::Error> {
        let tmp_config_path = tmp_dir.path().join(DEFAULT_CONFIG_FILE_NAME);
        if let Some(user_config) = user {
            user_config.to_file(&tmp_config_path)?;
        } else {
            File::create(&tmp_config_path)?;
        }

        Ok(tmp_config_path)
    }
}
