//! A launch: what every Claviron runner's launch says alike — its name, doors,
//! trust, request policy, static mounts, watching and workers — and the two
//! keys that are Lumenize's own. One flat `lumenize.yaml` holds both.
use std::path::{Path, PathBuf};

use claviron_runner::RunnerOptions;
use serde::{Deserialize, Serialize};

/// Deployment options; PHP engine and application policy stay in php.yaml.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "cli", derive(clap::Parser))]
#[cfg_attr(
    feature = "cli",
    command(
        about = "Serve a Laravel application through Claviron embedded PHP"
    )
)]
pub struct Options {
    /// The name, doors, trust, request policy, static mounts, watching and
    /// workers.
    #[cfg_attr(feature = "cli", command(flatten))]
    pub runner: RunnerOptions,

    /// Where Claviron's runtime configuration (`php.yaml`, `broadcast.yaml`,
    /// `logging.yaml`) is read from.
    #[cfg_attr(feature = "cli", arg(long, default_value = "config/runtime"))]
    pub config_dir: PathBuf,

    /// Broadcast application whose service is exposed to the native PHP
    /// broadcaster. Written in the file only.
    #[cfg(feature = "broadcast")]
    #[cfg_attr(feature = "cli", arg(skip))]
    pub broadcast_application: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            runner: RunnerOptions::default(),
            config_dir: "config/runtime".into(),
            #[cfg(feature = "broadcast")]
            broadcast_application: None,
        }
    }
}

/// Lumenize's own keys, beside the runner's in the same flat file, and what a
/// worker inherits of them.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct Php {
    pub(crate) config_dir: Option<PathBuf>,
    #[cfg(feature = "broadcast")]
    pub(crate) broadcast_application: Option<String>,
}

/// A launch as Lumenize writes and carries it.
pub(crate) type Launch = claviron_runner::launch::Launch<Php>;

impl From<Launch> for Options {
    fn from(launch: Launch) -> Self {
        Self {
            runner: launch.runner,
            config_dir: launch
                .own
                .config_dir
                .unwrap_or_else(|| "config/runtime".into()),
            #[cfg(feature = "broadcast")]
            broadcast_application: launch.own.broadcast_application,
        }
    }
}

impl From<Options> for Launch {
    fn from(options: Options) -> Self {
        Self {
            runner: options.runner,
            own: Php {
                config_dir: Some(options.config_dir),
                #[cfg(feature = "broadcast")]
                broadcast_application: options.broadcast_application,
            },
        }
    }
}

impl Options {
    pub fn from_file(path: impl AsRef<Path>) -> crate::Result<Self> {
        let launch: Launch =
            serde_yaml::from_str(&std::fs::read_to_string(path)?)?;
        Ok(launch.into())
    }

    /// What cannot be served as written, said before anything starts.
    pub fn validate(&self) -> crate::Result<()> {
        self.runner.validate()?;
        if self.runner.tcp.is_none()
            && self.runner.uds.is_none()
            && !self.runner.orbit
        {
            return Err("select tcp, uds or orbit ingress".into());
        }
        Ok(())
    }

    /// The file the command line names (`lumenize.yaml` by default), with the
    /// command line standing over it wherever it said anything. No listener
    /// or timeout is added that neither wrote.
    #[cfg(feature = "cli")]
    pub fn from_args() -> crate::Result<Self> {
        use clap::Parser;
        #[derive(Parser)]
        #[command(
            about = "Serve a Laravel application through Claviron embedded PHP"
        )]
        struct Args {
            #[arg(long, default_value = "lumenize.yaml")]
            config: PathBuf,
            #[arg(long)]
            config_dir: Option<PathBuf>,
            #[command(flatten)]
            runner: RunnerOptions,
        }
        let args = Args::parse();
        let written = Self::from_file(&args.config)?;
        let options = Self {
            runner: args.runner.over(written.runner),
            config_dir: args.config_dir.unwrap_or(written.config_dir),
            #[cfg(feature = "broadcast")]
            broadcast_application: written.broadcast_application,
        };
        options.validate()?;
        Ok(options)
    }
}

#[cfg(test)]
#[path = "../tests/options.rs"]
mod tests;
