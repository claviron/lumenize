//! One launch file: the common runner options and the hosted PHP application.
use std::path::{Path, PathBuf};

use claviron_runner::RunnerOptions;
use serde::{Deserialize, Serialize};

/// Deployment options. All runtime sections come from `config_path`, not a
/// directory of separate PHP, broadcast and logging files.
#[derive(Clone, Debug)]
pub struct Options {
    pub runner: RunnerOptions,
    pub config_path: PathBuf,
    pub configuration: Configuration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            runner: RunnerOptions::default(),
            config_path: "lumenize.yaml".into(),
            configuration: Configuration::default(),
        }
    }
}

/// Sections owned by their existing Claviron schemas. Retain their unresolved
/// values in the launch: each process resolves them against its boot snapshot.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    pub php: serde_yaml::Value,
    pub config_parser: PhpConfigParserOptions,
    pub cache: Option<serde_yaml::Value>,
    /// Claviron's `DbFileConfig`, resolved against each process's boot env.
    pub database: Option<serde_yaml::Value>,
    pub broadcasting: Option<serde_yaml::Value>,
    pub logging: Option<serde_yaml::Value>,
    /// The selected file's identity, carried to replacement workers.
    pub config_path: Option<PathBuf>,
}

/// Safety bound for returned PHP configuration arrays, not a PHP pool policy.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PhpConfigParserOptions {
    pub max_depth: usize,
}

impl Default for PhpConfigParserOptions {
    fn default() -> Self {
        Self { max_depth: default_config_max_depth() }
    }
}

fn default_config_max_depth() -> usize {
    64
}

pub(crate) type Launch = claviron_runner::launch::Launch<Configuration>;

impl From<Launch> for Options {
    fn from(launch: Launch) -> Self {
        Self {
            runner: launch.runner,
            config_path: launch
                .own
                .config_path
                .clone()
                .unwrap_or_else(|| "lumenize.yaml".into()),
            configuration: launch.own,
        }
    }
}

impl From<Options> for Launch {
    fn from(mut options: Options) -> Self {
        options.configuration.config_path = Some(options.config_path);
        Self { runner: options.runner, own: options.configuration }
    }
}

impl Options {
    pub fn from_file(path: impl AsRef<Path>) -> crate::Result<Self> {
        let path = path.as_ref();
        let mut options: Self =
            serde_yaml::from_str::<Launch>(&std::fs::read_to_string(path)?)?
                .into();
        options.config_path = path.to_owned();
        Ok(options)
    }

    pub fn validate(&self) -> crate::Result<()> {
        self.runner.validate()?;
        if self.config_path.as_os_str().is_empty() {
            return Err("config_path must name a file".into());
        }
        if self.configuration.php.is_null() {
            return Err("the launch must declare php.application".into());
        }
        #[cfg(not(feature = "broadcast"))]
        if self.configuration.broadcasting.is_some() {
            return Err("broadcasting requires the broadcast feature".into());
        }
        self.configuration.validate_database_features()?;
        if self.runner.tcp.is_none()
            && self.runner.uds.is_none()
            && !self.runner.orbit
        {
            return Err("select tcp, uds or orbit ingress".into());
        }
        Ok(())
    }

    /// The runner is not an edge and creates no access records.
    /// `logging: {path: ...}` is the single system-log shorthand; the full
    /// section may also declare application channels with the same file.
    pub(crate) fn system_log(
        &self
    ) -> crate::Result<Option<claviron::logging::SystemLogConfig>> {
        let Some(logging) = &self.configuration.logging else {
            return Ok(None);
        };
        let mapping =
            logging.as_mapping().ok_or("logging must be a mapping")?;
        if logging.get("path").is_none() {
            for key in mapping.keys() {
                if !matches!(key.as_str(), Some("system_log" | "channels")) {
                    return Err(format!("unknown logging field {key:?}; expected system_log or channels").into());
                }
            }
        }
        let config = if logging.get("path").is_some() {
            Some(logging)
        } else {
            logging.get("system_log").filter(|value| !value.is_null())
        };
        config
            .map(|value| {
                serde_yaml::from_value(value.clone()).map_err(Into::into)
            })
            .transpose()
    }

    #[cfg(feature = "cli")]
    pub fn from_args() -> crate::Result<Self> {
        use clap::Parser;
        // Re-exec workers receive the launch and do not reopen a default file.
        if let claviron::supervisor::ProcessEntry::Worker(worker) =
            claviron::supervisor::ProcessEntry::detect()?
        {
            return Ok(serde_json::from_slice::<Launch>(
                worker.launch().as_bytes(),
            )?
            .into());
        }
        #[derive(Parser)]
        #[command(
            about = "Serve a Laravel application through Claviron embedded PHP"
        )]
        struct Args {
            #[arg(long, alias = "config", default_value = "lumenize.yaml")]
            config_path: PathBuf,
            #[command(flatten)]
            runner: RunnerOptions,
        }
        let args = Args::parse();
        let mut written = Self::from_file(&args.config_path)?;
        written.runner = args.runner.over(written.runner);
        written.validate()?;
        Ok(written)
    }
}

impl Configuration {
    fn validate_database_features(&self) -> crate::Result<()> {
        let Some(database) = &self.database else {
            return Ok(());
        };
        if !cfg!(feature = "db") {
            return Err("database requires a database feature (postgres, mysql or sqlite)".into());
        }
        let mapping =
            database.as_mapping().ok_or("database must be a mapping")?;
        for (backend, enabled) in [
            ("postgres", cfg!(feature = "postgres")),
            ("mysql", cfg!(feature = "mysql")),
            ("sqlite", cfg!(feature = "sqlite")),
        ] {
            if mapping.contains_key(serde_yaml::Value::from(backend))
                && !enabled
            {
                return Err(format!(
                    "database.{backend} requires the {backend} feature"
                )
                .into());
            }
        }
        Ok(())
    }

    pub(crate) fn register(
        &self,
        state: &claviron::runtime::AppState,
        application: &str,
    ) -> claviron::runtime::registry::Result<()> {
        use claviron::runtime::registry;
        self.validate_database_features()
            .map_err(|error| registry::Error::msg(error.to_string()))?;
        #[cfg(feature = "db")]
        {
            // No separate db.yaml, even when the launch declares no native
            // connections. Application dependencies decide what is required.
            let database = self
                .database
                .as_ref()
                .map(|value| expand::<claviron::db::DbFileConfig>(state, value))
                .transpose()?
                .unwrap_or_default();
            claviron::db::register_with_config(state, database)?;
        }
        let yaml = serde_yaml::to_string(&self.php)
            .map_err(|error| registry::Error::msg(error.to_string()))?;
        claviron::ephp::register_application_from_yaml(
            state,
            application,
            &yaml,
        )?;
        // Registration declared the process INI above; no evaluation occurs
        // until configuration preparation, after all native extensions exist.
        let root = std::env::current_dir()
            .map_err(|error| registry::Error::msg(error.to_string()))?;
        let parser = claviron::laravel::congif::LaravelConfigParser::new(
            root,
            self.config_parser.max_depth,
        )
        .map_err(|error| registry::Error::msg(error.to_string()))?;
        state.config_reader().register(parser)?;

        let cache = self
            .cache
            .as_ref()
            .map(|value| expand::<claviron::cache::CacheConfig>(state, value))
            .transpose()?
            .unwrap_or_default();
        claviron::cache::register_with_config(state, cache)?;

        #[cfg(feature = "log")]
        {
            let mut logging = self.logging.clone().unwrap_or_else(|| {
                serde_yaml::Value::Mapping(Default::default())
            });
            if logging.get("path").is_some() {
                logging = serde_yaml::Value::Mapping(Default::default());
            }
            let map = logging
                .as_mapping_mut()
                .ok_or_else(|| registry::Error::msg("logging is a mapping"))?;
            map.entry("channels".into()).or_insert_with(|| {
                serde_yaml::Value::Mapping(Default::default())
            });
            let config = expand::<claviron::logging::pipeline::LoggingConfig>(
                state, &logging,
            )?;
            claviron::logging::pipeline::register_with_config(state, config)?;
        }
        #[cfg(not(feature = "log"))]
        if self
            .logging
            .as_ref()
            .is_some_and(|value| value.get("channels").is_some())
        {
            return Err(registry::Error::msg(
                "logging.channels requires the log feature",
            ));
        }

        #[cfg(feature = "broadcast")]
        {
            let mut applications = serde_yaml::Mapping::new();
            if let Some(broadcasting) = &self.broadcasting {
                let mut config =
                    broadcasting.as_mapping().cloned().ok_or_else(|| {
                        registry::Error::msg("broadcasting is a mapping")
                    })?;
                // Native publication needs no second app identity. External
                // Pusher REST clients can explicitly set their wire id.
                config.entry("id".into()).or_insert_with(|| application.into());
                applications.insert(application.into(), config.into());
            }
            let mut document = serde_yaml::Mapping::new();
            document.insert("applications".into(), applications.into());
            let config = expand::<claviron::broadcast::BroadcastConfig>(
                state,
                &document.into(),
            )?;
            claviron::broadcast::register_with_config(state, config)?;
            if self.broadcasting.is_some() {
                use claviron::laravel::broadcast::LaravelBroadcastProvider;
                use claviron::runtime::Register;

                LaravelBroadcastProvider::register(state)?;
            }
        }
        Ok(())
    }
}

fn expand<T: serde::de::DeserializeOwned>(
    state: &claviron::runtime::AppState,
    value: &serde_yaml::Value,
) -> claviron::runtime::registry::Result<T> {
    use claviron::runtime::registry;
    let raw = serde_yaml::to_string(value)
        .map_err(|error| registry::Error::msg(error.to_string()))?;
    let raw = claviron::helpers::yaml::expand_env_with(&raw, |key| {
        state.env_snapshot().get(key)
    })
    .map_err(|error| registry::Error::msg(error.to_string()))?;
    serde_yaml::from_str(&raw)
        .map_err(|error| registry::Error::msg(error.to_string()))
}

#[cfg(test)]
#[path = "../tests/options.rs"]
mod tests;
