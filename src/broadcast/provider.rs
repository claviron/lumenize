use std::sync::Arc;

use claviron::runtime::{AppState, Register, registry};
use claviron_php::embed::extension::register_extension;

use super::BroadcastExtension;

/// Connects PHP's native broadcaster to the Broadcast service.
/// Each PHP publication selects its catalog application by its own key.
///
/// The extension cannot run without the service, so registering it also
/// registers `BroadcastService` from the host's `broadcast.yaml` unless the
/// host has already registered one, for example from its own catalog.
pub struct LaravelBroadcastProvider;

impl Register<AppState> for LaravelBroadcastProvider {
    fn register(state: &AppState) -> registry::Result<()> {
        // Every role: occupancy runs in the master, sessions in the workers.
        claviron_broadcast::register(state)?;
        if state.is_master() {
            return Ok(());
        }
        register_extension(state, Arc::new(BroadcastExtension))
            .map_err(registry::Error::msg)
    }
}
