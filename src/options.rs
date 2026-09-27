use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use claviron::http::policy::{HttpPolicy, RequestPolicyOverride, ResponsePolicyOverride};
use serde::Deserialize;

/// Deployment options; PHP engine and application policy stay in php.yaml.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub name: Option<String>,
    pub config_dir: PathBuf,
    pub tcp: Option<SocketAddr>,
    pub uds: Option<PathBuf>,
    pub orbit: bool,
    /// Broadcast application whose service is exposed to the native PHP broadcaster.
    #[cfg(feature = "broadcast")]
    pub broadcast_application: Option<String>,
    /// Maximum accepted Orbit exchanges; waiting work remains in the visible lane.
    pub max_in_flight: usize,
    /// Shared request admission across all connections and ingress paths.
    #[serde(
        default,
        deserialize_with = "claviron::http::tuning::optional_count_limit"
    )]
    pub max_concurrent_requests: Option<claviron::http::policy::Limit<usize>>,
    pub requests: RequestPolicyOverride,
    pub responses: ResponsePolicyOverride,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            name: None,
            config_dir: "config/runtime".into(),
            tcp: None,
            uds: None,
            orbit: false,
            #[cfg(feature = "broadcast")]
            broadcast_application: None,
            max_in_flight: default_max_in_flight(),
            max_concurrent_requests: None,
            requests: Default::default(),
            responses: Default::default(),
        }
    }
}
fn default_max_in_flight() -> usize {
    claviron::orbit::transport::TRANSPORT_INBOX_CAPACITY
}
impl Options {
    pub fn from_file(path: impl AsRef<Path>) -> crate::Result<Self> {
        Ok(serde_yaml::from_str(&std::fs::read_to_string(path)?)?)
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.tcp.is_none() && self.uds.is_none() && !self.orbit {
            return Err("select tcp, uds or orbit ingress".into());
        }
        if self.max_in_flight == 0 {
            return Err("max_in_flight must be positive".into());
        }
        if self
            .max_concurrent_requests
            .and_then(|limit| limit.value())
            .is_some_and(|max| max == 0 || max > tokio::sync::Semaphore::MAX_PERMITS)
        {
            return Err(
                "max_concurrent_requests must be positive and within semaphore capacity, or off"
                    .into(),
            );
        }
        if self.uds.as_ref().is_some_and(|p| p.as_os_str().is_empty()) {
            return Err("uds path must not be empty".into());
        }
        Ok(())
    }

    pub(crate) fn policy(&self) -> HttpPolicy {
        HttpPolicy::unlimited().narrow(&claviron::http::policy::HttpPolicyOverride {
            requests: self.requests,
            responses: self.responses,
        })
    }

    /// CLI overrides the selected file. No implicit listener or timeout is added.
    #[cfg(feature = "cli")]
    pub fn from_args() -> crate::Result<Self> {
        use clap::Parser;
        #[derive(Parser)]
        #[command(about = "Serve a Laravel application through Claviron embedded PHP")]
        struct Args {
            #[arg(long, default_value = "lumenize.yaml")]
            config: PathBuf,
            #[arg(long)]
            name: Option<String>,
            #[arg(long)]
            config_dir: Option<PathBuf>,
            #[arg(long)]
            tcp: Option<SocketAddr>,
            #[arg(long)]
            uds: Option<PathBuf>,
            #[arg(long)]
            orbit: bool,
            #[arg(long)]
            max_in_flight: Option<usize>,
        }
        let args = Args::parse();
        let mut options = Self::from_file(&args.config)?;
        if args.name.is_some() {
            options.name = args.name;
        }
        if let Some(dir) = args.config_dir {
            options.config_dir = dir;
        }
        if args.tcp.is_some() {
            options.tcp = args.tcp;
        }
        if args.uds.is_some() {
            options.uds = args.uds;
        }
        options.orbit |= args.orbit;
        if let Some(limit) = args.max_in_flight {
            options.max_in_flight = limit;
        }
        options.validate()?;
        Ok(options)
    }
}
