# Lumenize

*For the small, fast spirit Lumen left behind.*

Lumenize brings Safe Atomic Rust Power to PHP/Laravel's breeze of magic.
A Laravel-focused PHP application server powered by Claviron.

**Development API. Publishing is disabled. The published `0.0.0` crate is a
name-reservation placeholder, not this implementation.**

## Application-owned entry

Your application owns its binary, Laravel code and runtime configuration.
Lumenize composes Claviron's standalone `server` lifecycle with embedded PHP.
Its current ingress adapters support HTTP/1 and HTTP/2 over TCP or a Unix
socket, and the same-host Orbit application transport. Select one or several.
No JavaScript engine or edge/vhost server is enabled by this composition.

```rust,no_run
use lumenize::{Options, Runner};
use lumenize::claviron::runtime::{AppState, registry};

fn configure(state: &AppState) -> registry::Result<()> {
    lumenize::register_router(state, |_state| {
        lumenize::axum::Router::new()
            .route("/rust", lumenize::axum::routing::get(|| async { "Hello from Rust" }))
    })
}

#[tokio::main]
async fn main() -> lumenize::Result<()> {
    Runner::new("web")
        .with_options(Options::from_file("lumenize.yaml")?)
        .run(configure)
        .await
}
```

The runner loads PHP through Claviron's existing `php.yaml` loader. The callback
registers the application's own Rust providers, services and routes. Providers
resolve dependencies during boot. `register_router(state, routes::router)` calls
the `FnOnce(AppState) -> Router` builder and installs the Rust
routes; undeclared paths and methods go to PHP. A matching Rust handler's response
is returned unchanged, including a 404. Pass a router without a custom fallback.
Omit route registration for a PHP-only application. The runner validates its
named PHP application before ingress starts.

For custom dispatch, `register_router_with(state, |php| service)` accepts a
cloneable Tower service over `AppRequest` and adds no fallback. The `PhpService`
handle can be used with `.fallback_service(php)`, mounted on a subtree, or called
explicitly. Its adapter is resolved during boot; neither registration API boots
or resolves PHP while the callback is running. Both APIs use the same ingress
admission and body guards.

```yaml
# lumenize.yaml, loaded from the application's working directory
name: web
config_dir: config/runtime
tcp: 127.0.0.1:8200
orbit: true
```

`config/runtime/php.yaml` declares the `applications.web` entry, its persistent
PHP script, environment, pool and execution policy. Paths in both files are
relative to the application's working directory. Laravel routes and business
logic remain PHP-owned. Shared Laravel integration remains in Claviron.

The optional `cli` Cargo feature exposes `Options::from_args()`: `--config`,
`--config-dir`, `--name`, `--tcp`, `--uds`, `--orbit` and `--max-in-flight`.
CLI flags override or enable entries in the selected file; use a separate
configuration file to disable an ingress. No listener is enabled implicitly.

## Boundaries

- A separate edge owns TLS, virtual hosts, static files and proxy trust policy.
  Socket ingress uses the physical connection's peer and plain HTTP scheme;
  it does not implicitly trust forwarded headers.
- `requests` and `responses` reuse Claviron's HTTP policy types. Request size
  and idle-read limits and response size and idle-write-source limits are
  enforced by its shared body guards, including on direct socket requests.
  Omitted policies are unbounded; declare the limits your deployment requires.
- `max_concurrent_requests` is a positive count or `off`; omission is unbounded.
  A single Claviron `Admission` is shared by all ingress paths and connections.
  `requests.queue_timeout` bounds admission waiting (503 on expiry), and
  `requests.head_timeout` bounds time to the response head (504 on expiry).
  The permit is released at the head; streaming replies retain their body policy.
- Keep Unix sockets on a host-local filesystem, outside a shared checkout.
- Persistent PHP uses the shared receive/respond protocol and Laravel Octane
  isolation. The current shared PHP service buffers the incoming request body
  before Laravel receives it, even when the transport reads it incrementally.
  CONNECT and unsupported native WebSocket upgrades are rejected with 501.
  Orbit upgrades require the optional Pusher service and a configured app key.
- Orbit uses Claviron's existing exchange/stream protocol and runtime identity
  `php`. All participants must agree on compiled transport geometry.
- Current process composition is standalone. Supervised workers, script-mode
  routing, an application-owned multihost edge and starter generation remain
  future work. These are independent of the existing PHP pool capacity.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and lifecycle contracts.

## Native Laravel broadcasting

The optional `broadcast` Cargo feature composes Claviron's existing Broadcast service
and Laravel PHP extension in the application process. Declare applications and
credentials in `config_dir/broadcast.yaml`, and select the PHP broadcaster's
application with `broadcast_application: your-application` in `lumenize.yaml`.
The secret must be present in the Rust process environment before bootstrap;
under systemd, supply it with `EnvironmentFile`.

Socket and Orbit ingress serve the configured `/app/{key}` WebSocket endpoints
and signed `/apps/{id}/events` REST endpoints. Orbit carries complete messages
over its existing SHM stream with native wakeups; no TCP or Unix socket listener
is needed in the application. The edge owns the public WebSocket handshake and
wire framing. Both ingress adapters run the same Claviron Broadcast session under
their own task ownership. Laravel's `claviron` broadcast driver calls
the native extension and publishes to the same Broadcast service, with no HTTP
client or separate Pusher process.

## Non-Affiliation

This independent project is not affiliated with or endorsed by Laravel,
Lumen, or the official Illuminate components. It is not a Laravel replacement.

## License

MIT.
