use std::collections::HashSet;

use cloudflare::framework::response::ApiFailure;

use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};

use crate::http;
use crate::settings::toml::Target;

pub mod bulk;
pub mod key;
pub mod namespace;

// TODO: callers outside this module should write their own error handling (lookin at you sites)
pub fn format_error(e: ApiFailure) -> String {
    http::format_error(e, Some(&kv_help))
}

// kv_help() provides more detailed explanations of Workers KV API error codes.
// See https://api.cloudflare.com/#workers-kv-namespace-errors for details.
fn kv_help(error_code: u16) -> &'static str {
    match error_code {
        7003 | 7000 => {
            "Your configuration file is likely missing the field \"account_id\", which is required to write to Workers KV."
        }
        // namespace errors
        10010 | 10011 | 10012 | 10013 | 10014 | 10018 => {
            "Run `wrangler kv:namespace list` to see your existing namespaces with IDs"
        }
        10009 => "Run `wrangler kv:key list` to see your existing keys", // key errors
        // TODO: link to more info
        // limit errors
        10022 | 10024 | 10030 => "See documentation",
        // TODO: link to tool for this?
        // legacy namespace errors
        10021 | 10035 | 10038 => "Consider moving this namespace",
        _ => "",
    }
}

pub fn validate_target(target: &Target) -> Result<(), failure::Error> {
    let mut missing_fields = Vec::new();

    if target.account_id.is_empty() {
        missing_fields.push("account_id")
    };

    if !missing_fields.is_empty() {
        failure::bail!(
            "Your configuration file is missing the following field(s): {:?}",
            missing_fields
        )
    } else {
        Ok(())
    }
}

fn check_duplicate_namespaces(target: &Target) -> bool {
    // HashSet for detecting duplicate namespace bindings
    let mut binding_names: HashSet<String> = HashSet::new();

    for namespace in &target.kv_namespaces {
        // Check if this is a duplicate binding
        if binding_names.contains(&namespace.binding) {
            return true;
        } else {
            binding_names.insert(namespace.binding.clone());
        }
    }

    false
}

// Get namespace id for a given binding name.
pub fn get_namespace_id(target: &Target, binding: &str) -> Result<String, failure::Error> {
    if check_duplicate_namespaces(&target) {
        failure::bail!(
            "Namespace binding \"{}\" is duplicated in \"{}\"",
            binding,
            target.name
        )
    }

    for namespace in &target.kv_namespaces {
        if namespace.binding == binding {
            return Ok(namespace.id.to_string());
        }
    }

    failure::bail!(
        "Namespace binding \"{}\" not found in \"{}\"",
        binding,
        target.name
    )
}

const KV_ASCII_SET: &AsciiSet = &CONTROLS.add(b'/');

fn url_encode_key(key: &str) -> String {
    utf8_percent_encode(key, KV_ASCII_SET).to_string()
}

#[cfg(test)]
mod tests {
    use crate::commands::kv;
    use crate::settings::toml::{KvNamespace, Target, TargetType};

    #[test]
    fn it_can_detect_duplicate_bindings() {
        let target_with_dup_kv_bindings = Target {
            account_id: "".to_string(),
            kv_namespaces: vec![
                KvNamespace {
                    id: "fake".to_string(),
                    binding: "KV".to_string(),
                },
                KvNamespace {
                    id: "fake".to_string(),
                    binding: "KV".to_string(),
                },
            ],
            name: "test-target".to_string(),
            main: "".to_string(),
            target_type: TargetType::Webpack,
            webpack_config: None,
            site: None,
            vars: None,
            text_blobs: None,
            usage_model: None,
        };
        assert!(kv::get_namespace_id(&target_with_dup_kv_bindings, "").is_err());
    }

    #[test]
    fn it_encodes_slash() {
        assert_eq!(kv::url_encode_key("/slash").to_string(), "%2Fslash");
    }

    #[test]
    fn it_doesnt_double_encode_slash() {
        assert_eq!(kv::url_encode_key("%2Fslash").to_string(), "%2Fslash");
    }
}
