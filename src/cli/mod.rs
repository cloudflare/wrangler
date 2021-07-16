pub mod build;
pub mod config;
pub mod dev;
pub mod generate;
pub mod init;
pub mod kv;
pub mod preview;
pub mod publish;
pub mod route;
pub mod secret;
pub mod subdomain;
pub mod tail;
pub mod whoami;
pub mod zone;

pub mod exec {
    pub use super::build::build;
    pub use super::config::configure;
    pub use super::dev::dev;
    pub use super::generate::generate;
    pub use super::init::init;
    pub use super::kv::kv_bulk;
    pub use super::kv::kv_key;
    pub use super::kv::kv_namespace;
    pub use super::preview::preview;
    pub use super::publish::publish;
    pub use super::route::route;
    pub use super::secret::secret;
    pub use super::subdomain::subdomain;
    pub use super::tail::tail;
    pub use super::whoami::whoami;
    pub use super::zone::zone;
}

use std::net::IpAddr;
use std::path::PathBuf;

use crate::commands::dev::Protocol;
use crate::preview::HttpMethod;
use crate::settings::toml::migrations::{
    DurableObjectsMigration, Migration, MigrationConfig, Migrations, RenameClass, TransferClass,
};
use crate::settings::toml::TargetType;

use clap::AppSettings;
use structopt::StructOpt;
use url::Url;

#[derive(Debug, Clone, StructOpt)]
#[structopt(
    name = "wrangler",
    author = "The Wrangler Team <wrangler@cloudflare.com>",
    setting = AppSettings::ArgRequiredElseHelp,
    setting = AppSettings::DeriveDisplayOrder,
    setting = AppSettings::VersionlessSubcommands,
)]
pub struct Cli {
    /// Toggle verbose output (when applicable)
    #[structopt(long, global = true)]
    pub verbose: bool,

    /// Path to configuration file.
    #[structopt(long, short = "c", default_value = "wrangler.toml", global = true)]
    pub config: PathBuf,

    /// Environment to perform a command on.
    #[structopt(name = "env", long, short = "e", global = true)]
    pub environment: Option<String>,

    #[structopt(subcommand)]
    pub command: Command,
}

#[derive(Debug, Clone, StructOpt)]
pub enum Command {
    /// Interact with your Workers KV Namespaces
    #[structopt(name = "kv:namespace", setting = AppSettings::SubcommandRequiredElseHelp)]
    KvNamespace(kv::KvNamespace),

    /// Individually manage Workers KV key-value pairs
    #[structopt(name = "kv:key", setting = AppSettings::SubcommandRequiredElseHelp)]
    KvKey(kv::KvKey),

    /// Interact with multiple Workers KV key-value pairs at once
    #[structopt(name = "kv:bulk", setting = AppSettings::SubcommandRequiredElseHelp)]
    KvBulk(kv::KvBulk),

    /// List or delete worker routes.
    #[structopt(name = "route", setting = AppSettings::SubcommandRequiredElseHelp)]
    Route(route::Route),

    /// Generate a secret that can be referenced in the worker script
    #[structopt(name = "secret", setting = AppSettings::SubcommandRequiredElseHelp)]
    Secret(secret::Secret),

    /// Generate a new worker project
    Generate {
        /// The name of your worker!
        #[structopt(index = 1, default_value = "worker")]
        name: String,

        /// A link to a GitHub template! Defaults to https://github.com/cloudflare/worker-template
        #[structopt(index = 2)]
        template: Option<String>,

        /// The type of project you want generated
        #[structopt(name = "type", long, short = "t")]
        target_type: Option<TargetType>,

        /// Initializes a Workers Sites project. Overrides 'type' and 'template'
        #[structopt(long, short = "s")]
        site: bool,
    },

    /// Create a wrangler.toml for an existing project
    Init {
        /// The name of your worker!
        #[structopt(index = 1)]
        name: Option<String>,

        /// The type of project you want generated
        #[structopt(name = "type", long, short = "t")]
        target_type: Option<TargetType>,

        /// Initializes a Workers Sites project. Overrides `type` and `template`
        #[structopt(long, short = "s")]
        site: bool,
    },

    /// Build your worker
    Build,

    /// Preview your code temporarily on cloudflareworkers.com
    Preview {
        /// Type of request to preview your worker with (get, post)
        #[structopt(index = 1, default_value = "get")]
        method: HttpMethod,

        /// URL to open in the worker preview
        #[structopt(short = "u", long, default_value = "https://example.com")]
        url: Url,

        /// Body string to post to your preview worker request
        #[structopt(index = 2)]
        body: Option<String>,

        /// Watch your project for changes and update the preview automagically
        #[structopt(long)]
        watch: bool,

        /// Don't open the browser on preview
        #[structopt(long)]
        headless: bool,
    },

    /// Start a local server for developing your worker
    Dev {
        /// Host to forward requests to, defaults to the zone of project or to
        /// tutorial.cloudflareworkers.com if unauthenticated.
        #[structopt(long, short = "h")]
        host: Option<String>,

        /// IP to listen on. Defaults to 127.0.0.1
        #[structopt(long, short = "i")]
        ip: Option<IpAddr>,

        /// Port to listen on. Defaults to 8787
        #[structopt(long, short = "p")]
        port: Option<u16>,

        /// Sets the protocol on which the wrangler dev listens, by default this is http
        /// but can be set to https
        #[structopt(name = "local-protocol")]
        local_protocol: Option<Protocol>,

        /// Sets the protocol on which requests are sent to the host, by default this is https
        /// but can be set to http
        #[structopt(name = "upstream-protocol")]
        upstream_protocol: Option<Protocol>,
    },

    /// Publish your worker to the orange cloud
    #[structopt(name = "publish")]
    Publish {
        /// [deprecated] alias of wrangler publish
        #[structopt(long, hidden = true)]
        release: bool,

        #[structopt(possible_value = "json")]
        output: Option<String>,

        #[structopt(flatten)]
        migration: AdhocMigration,
    },

    /// Authenticate Wrangler with a Cloudflare API Token or Global API Key
    #[structopt(name = "config")]
    Config {
        /// Use an email and global API key for authentication.
        /// This is not recommended; use API tokens (the default) if possible
        #[structopt(name = "api-key", long)]
        api_key: bool,
        /// Do not verify provided credentials before writing out Wrangler config file
        #[structopt(name = "no-verify", long)]
        no_verify: bool,
    },

    /// Configure your workers.dev subdomain
    #[structopt(name = "subdomain")]
    Subdomain {
        /// The subdomain on workers.dev you'd like to reserve
        #[structopt(name = "name", index = 1)]
        name: Option<String>,
    },

    /// Retrieve your user info and test your auth config
    #[structopt(name = "whoami")]
    Whoami,

    /// Aggregate logs from production worker
    #[structopt(name = "tail")]
    Tail {
        /// Specify an output format
        #[structopt(long, short = "f", default_value = "json", possible_values = &["json", "pretty"])]
        format: String,

        /// Port to accept tail log requests
        #[structopt(long = "port", short = "p")]
        tunnel_port: Option<u16>,

        /// Provides endpoint for cloudflared metrics. Used to retrieve tunnel url
        #[structopt(long = "metrics")]
        metrics_port: Option<u16>,
    },

    /// Authenticate Wrangler with your Cloudflare username and password
    #[structopt(name = "login")]
    Login,

    /// Report an error caught by wrangler to Cloudflare
    #[structopt(name = "report")]
    Report {
        /// Specifies a log to report (e.g. --log=1619728882567.log)
        #[structopt(name = "log", long)]
        log: Option<PathBuf>,
    },

    /// Fetch details on the specified Cloudflare zone
    #[structopt(name = "zone")]
    Zone {
        /// The zone to fetch information for
        #[structopt(index = 1, required = true)]
        zone: String,
    },
}

#[derive(Debug, Clone, StructOpt)]
pub struct AdhocMigration {
    /// Allow durable objects to be created from a class in your script
    #[structopt(name = "new-class", long, number_of_values = 1)]
    new_class: Vec<String>,

    /// Delete all durable objects associated with a class in your script
    #[structopt(name = "delete-class", long, number_of_values = 1)]
    delete_class: Vec<String>,

    /// Rename a durable object class
    #[structopt(name = "rename-class", long, number_of_values = 2)]
    rename_class: Vec<String>,

    /// Transfer all durable objects associated with a class in another script to a class in
    /// this script
    #[structopt(name = "transfer-class", long, number_of_values = 3)]
    transfer_class: Vec<String>,
}

impl AdhocMigration {
    pub fn into_migration_config(self) -> Option<MigrationConfig> {
        let migration = DurableObjectsMigration {
            new_classes: self.new_class,
            deleted_classes: self.delete_class,
            renamed_classes: self
                .rename_class
                .chunks_exact(2)
                .map(|chunk| {
                    let (from, to) = if let [from, to] = chunk {
                        (from.clone(), to.clone())
                    } else {
                        unreachable!("Chunks exact returned a slice with a length not equal to 2")
                    };

                    RenameClass { from, to }
                })
                .collect(),
            transferred_classes: self
                .transfer_class
                .chunks_exact(3)
                .map(|chunk| {
                    let (from_script, from, to) = if let [from_script, from, to] = chunk {
                        (from_script.clone(), from.clone(), to.clone())
                    } else {
                        unreachable!("Chunks exact returned a slice with a length not equal to 3")
                    };

                    TransferClass {
                        from,
                        from_script,
                        to,
                    }
                })
                .collect(),
        };
        let is_migration_empty = migration.new_classes.is_empty()
            && migration.deleted_classes.is_empty()
            && migration.renamed_classes.is_empty()
            && migration.transferred_classes.is_empty();

        if !is_migration_empty {
            Some(MigrationConfig {
                tag: None,
                migration: Migration {
                    durable_objects: migration,
                },
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rename_class(tag: &str) -> RenameClass {
        RenameClass {
            from: format!("renameFrom{}", tag),
            to: format!("renameTo{}", tag),
        }
    }

    fn transfer_class(tag: &str) -> TransferClass {
        TransferClass {
            from: format!("transferFromClass{}", tag),
            from_script: format!("transferFromScript{}", tag),
            to: format!("transferToClass{}", tag),
        }
    }

    #[test]
    fn adhoc_migration_parsing() {
        let command = Cli::from_iter(&[
            "wrangler",
            "publish",
            "--new-class",
            "newA",
            "--new-class",
            "newB",
            "--delete-class",
            "deleteA",
            "--delete-class",
            "deleteB",
            "--rename-class",
            "renameFromA",
            "renameToA",
            "--rename-class",
            "renameFromB",
            "renameToB",
            "--transfer-class",
            "transferFromScriptA",
            "transferFromClassA",
            "transferToClassA",
            "--transfer-class",
            "transferFromScriptB",
            "transferFromClassB",
            "transferToClassB",
        ])
        .command;

        if let Command::Publish { migration, .. } = command {
            assert_eq!(
                migration.into_migration_config(),
                Some(MigrationConfig {
                    tag: None,
                    migration: Migration {
                        durable_objects: DurableObjectsMigration {
                            new_classes: vec![String::from("newA"), String::from("newB")],
                            deleted_classes: vec![String::from("deleteA"), String::from("deleteB")],
                            renamed_classes: vec![rename_class("A"), rename_class("B")],
                            transferred_classes: vec![transfer_class("A"), transfer_class("B")],
                        }
                    }
                })
            );
        } else {
            assert!(false, "Unkown command {:?}", command)
        }
    }
}
