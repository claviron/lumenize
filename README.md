# Lumenize

*For the small, fast spirit Lumen left behind.*

Lumenize brings Safe Atomic Rust Power to PHP/Laravel's breeze of magic.
A Laravel-focused PHP application server powered by Claviron.

**Development API. Publishing is disabled. The published `0.0.0` crate is a
name-reservation placeholder, not this implementation.**

## Features

| Feature | Role |
| --- | --- |
| `axum` (default) | `register_router` and the shared Axum re-export |
| `cli` | Parse the application's command line |
| `tls` | HTTPS and optional mTLS on the TCP listener |
| `files` | Explicit static mounts; selects the Axum adapter |
| `broadcast` | Native Laravel broadcasting and socket endpoints; selects Axum |
| `watch` | Native source notifications and PHP application restarts for development |
| `metrics` | Claviron runtime metrics |
| `legacy-linux` | Compatibility with Linux kernels older than 5.3 |

`default-features = false` retains embedded PHP, TCP/UDS/Orbit ingress and
`register_router_with` for a raw Tower service. Neither Axum nor a file watcher
is selected by that base. `AppBody` follows the chosen adapter; raw callers can
construct it with `AppBody::new(body)` or `AppBody::empty()`. These features do
not select Claviron's edge, virtual hosts or another language runtime. TLS and
watch also work without Axum. CLI belongs to the application's binary and is
never enabled implicitly by another feature.

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

## Development source watching

Enable `watch` and `cli` in the application's Lumenize dependency, then start the
same application binary with `--watch`. For example:

```sh
cargo run --features lumenize/watch,lumenize/cli -- --config lumenize.dev.yaml --watch
```

A development composition can select its own runtime configuration directory:

```yaml
name: web
config_dir: config/development
tcp: 127.0.0.1:8200
watch: true
watch_paths: [app, routes, application.php]
```

`--watch-path` is repeatable and replaces `watch_paths` from YAML. Without an
explicit list, existing `app`, `bootstrap`, `config`, `php`, `resources`, `routes`
and `application.php` paths are watched. PHP source changes, including Blade PHP
templates and atomic editor saves, request a restart of this named PHP
application through Claviron's application manager. Existing work drains under
the PHP runtime's policy; listeners, Orbit ingress and Broadcast remain alive.
The pool may temporarily return 503 while it drains and starts again.

Use a **development PHP INI with OPcache disabled** (`opcache.enable=0` and
`opcache.enable_cli=0`), selected through `engine.ini_path` / `engine.ini_scan_dir`
in the development `php.yaml`. Worker replacement alone does not invalidate
cached bytecode. Watch does not change INI settings or clear Laravel's cached
configuration/routes; keep those caches disabled/cleared during development.
Rust changes require rebuilding. Runtime YAML, environment and INI changes need
process restart because the resolved runtime plan is immutable.

The watcher uses native filesystem events, without a polling or debounce timer.
It coalesces pending changes and waits for its native callback to close on
shutdown. Shared/network filesystems may not forward edits made on another
machine into the local kernel's notifications; use a local checkout or restart
explicitly in that case. No polling fallback is enabled.

## TLS on the application listener

Enable the optional `tls` Cargo feature to serve HTTPS on the TCP listener:

```yaml
tcp: 127.0.0.1:8443
tls_cert: config/tls/server.pem
tls_key: config/tls/server.key
# Optional: require a client certificate signed by this authority (mTLS).
tls_client_ca: config/tls/edge-ca.pem
```

With `cli`, the same fields are `--tls-cert`, `--tls-key` and `--tls-client-ca`.
They override the selected YAML file. Certificate and key must be supplied
together, and TLS requires `tcp`. Invalid material fails before listeners open.
One certificate serves this application; there is no vhost or SNI certificate
selection. HTTP/2 and HTTP/1.1 are advertised through ALPN. UDS and Orbit are
unaffected. Laravel receives the physical HTTPS scheme and TLS connection facts.
Certificates are loaded at startup; restart the process after renewal.
A pending TLS handshake is cancelled by listener shutdown; no handshake deadline
is invented.

## Static assets

The optional `files` Cargo feature exposes Claviron's static file service without
an edge dependency. Declare explicit mounts in `lumenize.yaml`:

```yaml
files:
  root: public
  statics:
    - path: /build
      dir: build
```

GET/HEAD assets are served before application routes on every ingress, including
Orbit. Other methods and unmatched mount prefixes reach the application. A missing
file inside a mount returns 404; set `none_found: true` on that mount to fall
through instead. PHP sources, dotfiles and traversal paths retain the shared file
service's protections. Mount `rules` use Claviron's existing caching policy;
`config_dir/files.yaml` owns the shared cache configuration and reload behavior.
No general filesystem fallback or directory index is enabled.

## Boundaries

- A separate edge owns virtual hosts and proxy trust policy. Static assets
  may be served by that edge or the application’s optional `files` feature.
  Socket ingress uses the physical connection's peer and HTTP/HTTPS scheme;
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
  Orbit upgrades require the optional Broadcast service and a configured app key.
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
