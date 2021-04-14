use std::collections::HashMap;
use std::env;

use config::{ConfigError, Source, Value};

const PREFIX_PATTERN: &str = "CF_";

pub trait QueryEnvironment {
    fn get_var(&self, var: &'static str) -> Result<String, std::env::VarError>;

    fn empty(&self) -> Result<bool, failure::Error>;
}

#[derive(Clone, Debug)]
pub struct Environment {
    whitelist: Vec<&'static str>,
}

impl Environment {
    pub fn with_whitelist(whitelist: Vec<&'static str>) -> Self {
        Environment { whitelist }
    }
}

impl QueryEnvironment for Environment {
    fn get_var(&self, var: &'static str) -> Result<String, std::env::VarError> {
        env::var(var)
    }

    fn empty(&self) -> Result<bool, failure::Error> {
        let env = self.collect()?;

        Ok(env.is_empty())
    }
}

// Source trait implementation for use with Config::merge
// until config crate removal is complete. This is effectively a
// copy of the config crate's impl of Source for its Environment
// struct, but rather than pulling the whole environment in #collect,
// we pull in only whitelisted values, and rather than taking a custom
// prefix, we assume a prefix of `CF_`.
impl Source for Environment {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new((*self).clone())
    }

    fn collect(&self) -> Result<HashMap<String, Value>, ConfigError> {
        let mut m = HashMap::new();
        let uri: String = "env".into();

        for key in &self.whitelist {
            if let Ok(value) = env::var(key) {
                // remove the `CF` prefix before adding to collection
                let key = key.strip_prefix(PREFIX_PATTERN).unwrap_or(key);
                m.insert(key.to_lowercase(), Value::new(Some(&uri), value));
            }
        }

        Ok(m)
    }
}

#[derive(Clone, Debug, Default)]
#[cfg(test)]
pub struct MockEnvironment {
    vars: Vec<(&'static str, &'static str)>,
}

#[cfg(test)]
impl MockEnvironment {
    pub fn set(&mut self, key: &'static str, value: &'static str) -> &Self {
        self.vars.push((key, value));

        self
    }
}

#[cfg(test)]
impl QueryEnvironment for MockEnvironment {
    #[allow(unused_variables)]
    fn get_var(&self, var: &'static str) -> Result<String, std::env::VarError> {
        if self.vars.iter().any(|&(key, _)| key == var) {
            Ok("Some Mocked Result".to_string()) // Returns a mocked response
        } else {
            Err(std::env::VarError::NotPresent)
        }
    }

    fn empty(&self) -> Result<bool, failure::Error> {
        Ok(self.vars.is_empty())
    }
}

// config::Source trait implementation for use with config::Config.merge
// until config crate removal is complete.
#[cfg(test)]
impl Source for MockEnvironment {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new((*self).clone())
    }

    fn collect(&self) -> Result<HashMap<String, Value>, ConfigError> {
        let mut m = HashMap::new();
        let uri: String = "env".into();

        for (key, value) in &self.vars {
            // remove the `CF` prefix before adding to collection
            let prefix_pattern = "CF_";
            let key = &key[prefix_pattern.len()..];

            m.insert(key.to_lowercase(), Value::new(Some(&uri), *value));
        }

        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_gets_from_the_environment() {
        env::set_var("CF_API_KEY", "waylongapikey");
        env::set_var("CF_EMAIL", "user@example.com");
        env::set_var("CF_IRRELEVANT", "irrelevant");

        let environment = Environment::with_whitelist(vec!["CF_API_KEY", "CF_EMAIL"]);

        let mut expected_env_vars: HashMap<String, Value> = HashMap::new();

        // we expect that our environment variables will be stripped of the
        // `CF_` prefix, and that they will be downcased; consistent with the
        // behavior of `config::Environment::with_prefix("CF")`
        expected_env_vars.insert(
            "api_key".to_string(),
            Value::new(Some(&"env".to_string()), "waylongapikey"),
        );
        expected_env_vars.insert(
            "email".to_string(),
            Value::new(Some(&"env".to_string()), "user@example.com"),
        );

        assert_eq!(environment.collect().unwrap(), expected_env_vars);
    }
}
