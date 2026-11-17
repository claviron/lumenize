//! Laravel integration for applications that own a Claviron Edge.

use claviron::runtime::{AppState, Register, registry};

/// Install the Laravel config parser and, with `broadcast`, the native
/// broadcaster and its Edge routes. Call during bootstrap after Edge has
/// registered its host registry. This does not start a PHP application.
///
/// A host with its own broadcast catalog can register it before this call.
/// The auth event relay remains available through `crate::auth::register`.
pub fn register(state: &AppState) -> registry::Result<()> {
    #[cfg(feature = "broadcast")]
    {
        claviron_broadcast::edge::register(state)?;
        crate::LaravelBroadcastProvider::register(state)?;
    }
    crate::LaravelConfigProvider::register(state)
}
