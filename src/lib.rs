//! Laravel integration for Claviron hosts, independent of their boot composition.
//!
//! Authentication, broadcasting, PHP configuration evaluation and console
//! execution each have their own module and registration boundary.
//! Importing this library does not register services or start a PHP application.
//! The optional `runner` feature adds a single-application serving composition.
//! `cli` supplies an independent Artisan entry and, with `runner`, launch parsing.

pub use claviron;
#[cfg(feature = "axum")]
pub use claviron::axum;
#[cfg(all(feature = "runner", feature = "axum"))]
pub use claviron::http::routing::{Routable, register_routable};
#[cfg(feature = "runner")]
pub use claviron::php::PhpRunner;
#[cfg(all(feature = "runner", feature = "axum"))]
pub use claviron::runner::register_router;
#[cfg(feature = "runner")]
pub use claviron::runner::{
    AppRequest, AppResponse, Owner, register_router_with,
};
#[cfg(feature = "runner")]
pub use claviron::runtime::{FleetName, Register, WorkerCount};
#[cfg(feature = "runner")]
pub use options::{Configuration, Options, PhpConfigParserOptions};
#[cfg(feature = "runner")]
pub use runner::Runner;

#[cfg(feature = "runner")]
pub type Error = Box<dyn std::error::Error + Send + Sync>;
#[cfg(feature = "runner")]
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(feature = "runner")]
mod application;
pub mod artisan;
#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "bench")]
pub mod bench;
#[cfg(feature = "broadcast")]
pub mod broadcast;
#[cfg(feature = "cli")]
pub mod cli;
pub mod congif;
#[cfg(feature = "runner")]
mod options;
#[cfg(feature = "runner")]
mod runner;
#[cfg(feature = "watch")]
mod watch;
