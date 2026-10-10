//! Laravel applications on Claviron's embedded PHP runtime.
//!
//! The doors, routing, guards, static mounts, broadcasting, watching and
//! workers are Claviron's runner shell (`claviron_runner`); Lumenize supplies
//! the owner — the launch's PHP application — and what is Laravel's.
#![forbid(unsafe_code)]
mod application;
mod options;
mod runner;
#[cfg(feature = "watch")]
mod watch;

pub use claviron;
#[cfg(feature = "axum")]
pub use claviron::axum;
/// A host-free routable: its routes stand behind the application's own and in
/// front of PHP. Register it with `register_routable`.
#[cfg(feature = "axum")]
pub use claviron::http::routing::{Routable, register_routable};
pub use claviron::runtime::{FleetName, Register, WorkerCount};
#[cfg(feature = "axum")]
pub use claviron_runner::register_router;
pub use claviron_runner::{
    AppRequest, AppResponse, Owner, register_router_with,
};
pub use options::{Configuration, Options, PhpConfigParserOptions};
pub use runner::Runner;
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;
