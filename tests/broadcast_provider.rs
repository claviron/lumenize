// Its own test binary: PHP's extension table is process-wide, and the
// provider adds the broadcast extension to it.

use claviron::runtime::{AppState, Register};
use claviron_broadcast::BroadcastService;
use lumenize::broadcast::LaravelBroadcastProvider;

// The provider is the host's one registration: the service the extension
// publishes through comes with it.
#[test]
fn laravel_broadcast_provider_registers_its_service() {
    let root = tempfile::tempdir().unwrap();
    let state = AppState::standalone("broadcast-provider-test", root.path());
    LaravelBroadcastProvider::register(&state).unwrap();
    assert!(state.registry_ref().resolve::<BroadcastService>().is_some());
}
