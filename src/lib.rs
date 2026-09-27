//! Laravel applications on Claviron's embedded PHP runtime.
#![forbid(unsafe_code)]
mod application;
#[cfg(feature = "broadcast")]
mod broadcast;
mod options;
mod orbit;
mod routing;
mod runner;
mod socket;

pub use claviron::{self, axum};
pub use options::Options;
pub use routing::{AppRequest, AppResponse, PhpService, register_router, register_router_with};
pub use runner::Runner;
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
