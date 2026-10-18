# Lumenize

Laravel integration for applications hosted by Claviron.

Keep your Laravel routes, controllers, models and notifications. Lumenize connects
them to a native application host, with optional shared cache, sessions, queues
and broadcasting. You do not need to write Rust to use the PHP integration.

## Start with Laravel

Install the PHP adapters in your application:

```sh
composer require claviron/laravel:0.1.0
```

Composer installs the Laravel integration and its `claviron/sapi` dependency.
It does not install the native host or the `lumenize` executable; those are
prepared separately. Once the executable is installed, run Artisan commands
from your application directory:

```sh
lumenize list
```

Your application's configuration and authorization remain yours. Enable only
the services your host provides; installing the package does not start a server
or replace your existing application. Start with the [PHP guide](php/README.md).

## Packages

The library separates framework integration from its optional application
runner. An application may use the integration APIs while retaining its own
Claviron boot, Edge hosts, configuration, providers and process lifecycle.

| Package | Responsibility |
| --- | --- |
| `lumenize` | Laravel integration, Artisan API, and opt-in `runner` and `cli` capabilities |
| [`lumenize-cli`](cli/README.md) | Thin `lumenize` Artisan console binary; no HTTP Runner dependency |
| [`claviron/laravel`](php/README.md) (Composer) | Laravel drivers, Octane receive/respond adapter, providers, queue and event integration |

The Rust packages are development implementations with publishing disabled.
The published `lumenize` name-reservation placeholder is not this implementation.

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

## Extend with Rust (optional)

Rust lets you add native services or build a host tailored to your application.
Start with the ready-made Runner below when you want the supplied server
composition. Direct integration is for applications that already own a host:
it adds Laravel support without starting a second server.

### Use the library without the runner

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

### Ready-made application composition

Enable `runner` when the application wants `lumenize::{Options, Runner}` instead
of composing its own Edge. The downstream application still owns its `main.rs`.
See the [Runner guide](docs/runner/README.md) for socket/Orbit ingress, routing and workers.

```toml
lumenize = { version = "0.2.0-alpha.18", features = ["runner", "cli", "axum"] }
```

`runner` and `cli` are independent. `cli` alone provides `lumenize::cli::run()`
for Artisan without a server or supervisor; `runner + cli` also enables
`Options::from_args()`. Neither is enabled by default. The separate executable
package selects only `cli` and does not own the reusable Runner API.

## License

MIT. This independent project is not affiliated with or endorsed by Laravel
or its official components.
