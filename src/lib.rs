//! Laravel applications on Claviron's embedded PHP runtime.
#![forbid(unsafe_code)]
mod application;
#[cfg(feature = "broadcast")]
mod broadcast;
#[cfg(feature = "files")]
mod files;
mod options;
mod orbit;
mod routing;
mod runner;
mod socket;
#[cfg(feature = "tls")]
mod tls;
#[cfg(feature = "watch")]
mod watch;

pub use claviron;
#[cfg(feature = "axum")]
pub use claviron::axum;
#[cfg(feature = "files")]
pub use files::Files;
pub use options::Options;
#[cfg(feature = "axum")]
pub use routing::register_router;
pub use routing::{AppBody, AppRequest, AppResponse, PhpService, register_router_with};
pub use runner::Runner;
#[cfg(feature = "tls")]
pub use tls::Tls;
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
