//! Laravel integration for Claviron hosts, independent of their boot composition.
//!
//! Authentication, broadcasting, PHP configuration evaluation and console
//! execution each have their own module and registration boundary.
//! Importing this library does not register services or start a PHP application.
//! The ready-made runner is a separate `lumenize-runner` package.

pub use claviron;

pub mod artisan;
#[cfg(feature = "auth")]
pub mod auth;
#[cfg(feature = "broadcast")]
pub mod broadcast;
pub mod congif;
