//! Laravel applications on Claviron's embedded PHP runtime.
//!
//! The doors, routing, guards, static mounts, broadcasting, watching and
//! workers are Claviron's runner shell (`claviron::runner`); Lumenize supplies
//! the owner — the named php.yaml application — and what is Laravel's.
#![forbid(unsafe_code)]
mod application;
mod options;
mod runner;
#[cfg(feature = "watch")]
mod watch;

pub use claviron;
#[cfg(feature = "axum")]
pub use claviron::axum;
#[cfg(feature = "axum")]
pub use claviron::runner::register_router;
pub use claviron::runner::{AppRequest, AppResponse, Owner, register_router_with};
pub use options::Options;
pub use runner::Runner;
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
