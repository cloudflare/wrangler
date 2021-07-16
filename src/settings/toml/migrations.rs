use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum MigrationTag {
    HasTag(String),
    NoTag,
    NoScript,
    Unknown,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum Migrations {
    Adhoc {
        // The actual existing migration tag for the script, filled in by
        // the publish command before form building. Used to validate provided_old_tag
        script_tag: MigrationTag,
        // The expected migration tag for the script if provided on the command line using --old-tag.
        provided_old_tag: Option<String>,
        // The new migration tag for the script if provided on the command line using --new-tag.
        new_tag: Option<String>,
        // The actual migration, if any directives are provided
        migration: Option<Migration>,
    },
    List {
        // The actual existing migration tag for the script, filled in by
        // the publish command before form building
        script_tag: MigrationTag,
        migrations: Vec<MigrationConfig>,
    },
}

impl Migrations {
    fn validate(&self) -> Result<(), anyhow::Error> {
        // validate migration tags
        match &self {
            Migrations::Adhoc {
                script_tag: MigrationTag::HasTag(script_tag),
                provided_old_tag: Some(provided_tag),
                ..
            } if provided_tag != script_tag => anyhow::bail!(
                "The migration tag provided with your migration (\"{}\") does not match the script's current migration tag of \"{}\", \
                please check that you are applying the correct migration", provided_tag, script_tag
            ),
            Migrations::Adhoc {
                script_tag: MigrationTag::NoTag,
                provided_old_tag: Some(_),
                ..
            } => anyhow::bail!("You provided an existing migration tag with your migration but the script does not have a migration tag set."),
            Migrations::Adhoc {
                script_tag: MigrationTag::NoScript,
                provided_old_tag: Some(_),
                ..
            } => anyhow::bail!("You provided an existing migration tag with your migration but the script does not exist."),
            Migrations::Adhoc {
                script_tag: MigrationTag::HasTag(existing_tag),
                provided_old_tag: None,
                ..
            } => anyhow::bail!("You didn't provide an existing tag with your migration, but the script currently has a migration tag set to \"{}\"", existing_tag),
            Migrations::Adhoc {
                provided_old_tag: Some(_),
                new_tag: None,
                ..
            } => anyhow::bail!("You must provide a --new-tag when --old-tag is set."),
            Migrations::Adhoc {
                script_tag: MigrationTag::Unknown, ..
            } => anyhow::bail!("The migration tag for the script was never fetched. This is a bug in wrangler, and we'd appreciate an issue report!"),
            Migrations::List {
                script_tag: MigrationTag::Unknown, ..
            } => anyhow::bail!("The migration tag for the script was never fetched. This is a bug in wrangler, and we'd appreciate an issue report!"),
            _ => ()
        };

        // validate migration list
        if let Migrations::List { migrations, .. } = &self {
            let mut seen = HashSet::new();
            for migration in migrations {
                if seen.contains(&migration.tag) {
                    anyhow::bail!("The migration tag \"{}\" appears multiple times in the list of migrations in your wrangler.toml", migration.tag);
                }
                seen.insert(&migration.tag);
            }
        };

        Ok(())
    }

    pub fn api_migration(&self) -> Result<Option<ApiMigration>, anyhow::Error> {
        self.validate()?;
        match &self {
            Migrations::Adhoc {
                provided_old_tag,
                new_tag,
                migration,
                ..
            } => {
                let api_migration = ApiMigration {
                    old_tag: provided_old_tag.to_owned(),
                    new_tag: new_tag.to_owned(),
                    steps: migration
                        .as_ref()
                        .map_or_else(Vec::new, |m| vec![m.clone()]),
                };

                log::info!("API Migration: {:#?}", api_migration);

                Ok(Some(api_migration))
            }
            Migrations::List {
                script_tag,
                migrations,
            } => {
                let mut migrations = migrations.clone();
                let migrations = match script_tag {
                    MigrationTag::HasTag(tag) => {
                        let position =
                            migrations
                                .iter()
                                .position(|m| &m.tag == tag)
                                .ok_or_else(|| {
                                    anyhow::format_err!(
                                        "The script's current migration tag of \"{}\" was not \
                                found in the list of migrations in wrangler.toml",
                                        tag
                                    )
                                })?;
                        migrations.drain(0..=position);
                        migrations
                    }
                    _ => migrations,
                };

                if migrations.is_empty() {
                    return Ok(None);
                }

                let old_tag = match script_tag {
                    MigrationTag::HasTag(old_tag) => Some(old_tag.clone()),
                    _ => None,
                };
                let new_tag = migrations.last().map(|m| m.tag.clone());
                let steps = migrations.into_iter().map(|m| m.migration).collect();

                let api_migration = ApiMigration {
                    old_tag,
                    new_tag,
                    steps,
                };

                log::info!("API Migration: {:#?}", api_migration);

                Ok(Some(api_migration))
            }
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct MigrationConfig {
    pub tag: String,
    #[serde(flatten)]
    pub migration: Migration,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ApiMigration {
    #[serde(skip_serializing_if = "Option::is_none")]
    old_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new_tag: Option<String>,
    steps: Vec<Migration>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct Migration {
    #[serde(flatten)]
    pub durable_objects: DurableObjectsMigration,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct DurableObjectsMigration {
    #[serde(default)]
    pub new_classes: Vec<String>,
    #[serde(default)]
    pub deleted_classes: Vec<String>,
    #[serde(default)]
    pub renamed_classes: Vec<RenameClass>,
    #[serde(default)]
    pub transferred_classes: Vec<TransferClass>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RenameClass {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TransferClass {
    pub from: String,
    pub from_script: String,
    pub to: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adhoc_only_change_tag() -> Result<(), anyhow::Error> {
        let migrations = Migrations::Adhoc {
            script_tag: MigrationTag::HasTag(String::from("v1")),
            provided_old_tag: Some(String::from("v1")),
            new_tag: Some(String::from("v2")),
            migration: None,
        };

        let api_migration = migrations.api_migration()?;

        assert_eq!(
            api_migration,
            Some(ApiMigration {
                old_tag: Some(String::from("v1")),
                new_tag: Some(String::from("v2")),
                steps: vec![]
            })
        );

        Ok(())
    }

    #[test]
    fn adhoc_full() -> Result<(), anyhow::Error> {
        let migrations = Migrations::Adhoc {
            script_tag: MigrationTag::HasTag(String::from("v1")),
            provided_old_tag: Some(String::from("v1")),
            new_tag: Some(String::from("v2")),
            migration: Some(Migration {
                durable_objects: DurableObjectsMigration {
                    new_classes: vec![String::from("A")],
                    deleted_classes: vec![],
                    renamed_classes: vec![],
                    transferred_classes: vec![],
                },
            }),
        };

        let api_migration = migrations.api_migration()?;

        assert_eq!(
            api_migration,
            Some(ApiMigration {
                old_tag: Some(String::from("v1")),
                new_tag: Some(String::from("v2")),
                steps: vec![Migration {
                    durable_objects: DurableObjectsMigration {
                        new_classes: vec![String::from("A")],
                        deleted_classes: vec![],
                        renamed_classes: vec![],
                        transferred_classes: vec![],
                    }
                }]
            })
        );

        Ok(())
    }

    #[test]
    fn migration_list_fresh() -> Result<(), anyhow::Error> {
        let migrations = Migrations::List {
            script_tag: MigrationTag::NoScript,
            migrations: vec![MigrationConfig {
                tag: String::from("v1"),
                migration: Migration {
                    durable_objects: DurableObjectsMigration {
                        new_classes: vec![String::from("A")],
                        deleted_classes: vec![],
                        renamed_classes: vec![],
                        transferred_classes: vec![],
                    },
                },
            }],
        };
        let api_migration = migrations.api_migration()?;

        assert_eq!(
            api_migration,
            Some(ApiMigration {
                old_tag: None,
                new_tag: Some(String::from("v1")),
                steps: vec![Migration {
                    durable_objects: DurableObjectsMigration {
                        new_classes: vec![String::from("A")],
                        deleted_classes: vec![],
                        renamed_classes: vec![],
                        transferred_classes: vec![],
                    }
                }]
            })
        );

        Ok(())
    }

    #[test]
    fn migration_list_hastag_noop() -> Result<(), anyhow::Error> {
        let migrations = Migrations::List {
            script_tag: MigrationTag::HasTag(String::from("v1")),
            migrations: vec![MigrationConfig {
                tag: String::from("v1"),
                migration: Migration {
                    durable_objects: DurableObjectsMigration {
                        new_classes: vec![String::from("A")],
                        deleted_classes: vec![],
                        renamed_classes: vec![],
                        transferred_classes: vec![],
                    },
                },
            }],
        };
        let api_migration = migrations.api_migration()?;

        assert_eq!(api_migration, None);

        Ok(())
    }

    #[test]
    fn migration_list_hastag() -> Result<(), anyhow::Error> {
        let migrations = Migrations::List {
            script_tag: MigrationTag::HasTag(String::from("v1")),
            migrations: vec![
                MigrationConfig {
                    tag: String::from("v1"),
                    migration: Migration {
                        durable_objects: DurableObjectsMigration {
                            new_classes: vec![String::from("A")],
                            deleted_classes: vec![],
                            renamed_classes: vec![],
                            transferred_classes: vec![],
                        },
                    },
                },
                MigrationConfig {
                    tag: String::from("v2"),
                    migration: Migration {
                        durable_objects: DurableObjectsMigration {
                            new_classes: vec![String::from("B")],
                            deleted_classes: vec![],
                            renamed_classes: vec![],
                            transferred_classes: vec![],
                        },
                    },
                },
            ],
        };
        let api_migration = migrations.api_migration()?;

        assert_eq!(
            api_migration,
            Some(ApiMigration {
                old_tag: Some(String::from("v1")),
                new_tag: Some(String::from("v2")),
                steps: vec![Migration {
                    durable_objects: DurableObjectsMigration {
                        new_classes: vec![String::from("B")],
                        deleted_classes: vec![],
                        renamed_classes: vec![],
                        transferred_classes: vec![],
                    }
                }]
            })
        );

        Ok(())
    }
}
