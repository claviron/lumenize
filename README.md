# Lumenize

Laravel integration for applications hosted by Claviron.

This repository separates framework integration from the ready-made application
runner. An application may use the integration library while retaining its own
Claviron boot, Edge hosts, configuration, providers and process lifecycle.

| Package | Responsibility |
| --- | --- |
| `lumenize` | Laravel configuration evaluation, opt-in auth events and native broadcasting, and the Artisan execution API |
| [`lumenize-runner`](runner/README.md) | Launch options, PHP owner, socket/Orbit ingress composition, source watching and the `lumenize` console binary |
| [`claviron/laravel`](php/README.md) (Composer) | Laravel drivers, Octane receive/respond adapter, providers, queue and event integration |

The Rust packages are development implementations with publishing disabled.
The published `lumenize` name-reservation placeholder is not this implementation.

## Use the library without the runner

The library has no default features. Configuration evaluation and
`artisan::run` are available directly; `auth` enables the auth-event adapter and
`broadcast` enables the native broadcast adapter. These features compile
capabilities; they do not register services or start an application.

Register the needed adapters in the host's existing composition, before
configuration preparation and PHP execution:

```rust,no_run
use claviron::runtime::{AppState, Register, registry};
use lumenize::broadcast::LaravelBroadcastProvider;
use lumenize::congif::LaravelConfigProvider;

// Requires lumenize's `auth` and `broadcast` features. The host has already
// declared its PHP engine/INI, broadcast service and event transport.
fn register_laravel(state: &AppState) -> registry::Result<()> {
    LaravelConfigProvider::register(state)?;
    LaravelBroadcastProvider::register(state)?;
    lumenize::auth::register(state)
}
```

The host owns credential admission, guards, authorization, route selection and
runtime resources. It chooses which registrations to perform and in what
order. Lumenize introduces no second `AppState`, PHP engine, supervisor,
listener or boot sequence. Claviron does not depend on Lumenize.

Configuration evaluation uses trusted PHP and the host's existing SAPI;
it is not a sandbox. Auth events are successful state-transition facts,
not a new authentication mechanism. Broadcasting retains the existing
`claviron_laravel_broadcast` function and protocol behavior.

## PHP integration

Install the companion Composer package `claviron/laravel`. Its PHP integration
classes use `Lumenize\`, independently of the engine's `Claviron\` classes.
Driver names, event topics and native functions are unchanged. The package
continues to consume `claviron/sapi`, supplied by Claviron PHP Embed.

Update imports, explicit providers and Composer discovery metadata together;
there is no old-namespace alias. Regenerate autoload/provider/config caches and
drain or migrate queued payloads containing the former PHP class names before
switching running workers. Native `Claviron\Runtime` envelopes stay unchanged.

Laravel owns its container, models, authorization, notifications, queues and
request semantics. The host may run an ordinary front controller in Script
mode or select the package's Octane adapter in Application mode. See the
[integration guide](docs/README.md) and [Composer documentation](php/README.md).

## Ready-made application composition

Use `lumenize-runner` when the application wants the existing `Runner` and
`Options` API instead of composing its own Edge. The downstream application
still owns its `main.rs`; the runner package also carries the separate Artisan
console entry. The integration library never depends on that package.

## License

MIT. This independent project is not affiliated with or endorsed by Laravel
or its official components.
