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
| `log` | `logging.channels` for Laravel's `claviron` log driver |
| `log-otlp` | `log` plus the OTLP channel adapter |
| `metrics` | Claviron runtime metrics |
| `db` | Native process-local DB composition |
| `postgres`, `mysql`, `sqlite` | Select the corresponding native DB backend; each enables `db` |
| `legacy-linux` | Compatibility with Linux kernels older than 5.3 |

The doors, guards, routing, static mounts, broadcasting, watching and workers
are Claviron's runner shell (`claviron_runner`), shared with the Python
runner; `workers: N` in `lumenize.yaml` serves the application from N worker
processes. `default-features = false` retains embedded PHP, TCP/UDS/Orbit ingress and
`register_router_with` for a raw Tower service. Neither Axum nor a file watcher
is selected by that base. A raw service takes the shell's `AppRequest`, whose
body is `RequestBody` (`RequestBody::new(body)`, `RequestBody::empty()`). These features do
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
        .with_bootstrap(configure)
        .run()
        .await
}
```

The runner reads one launch document and reuses Claviron's existing PHP schema. The callback
registers the application's own Rust providers, services and routes. Providers
resolve dependencies during boot. `register_router(state, routes::router)` calls
the `FnOnce(AppState) -> Router` builder and installs the Rust
routes; undeclared paths and methods go to PHP. A matching Rust handler's response
is returned unchanged, including a 404. Pass a router without a custom fallback.
Omit route registration for a PHP-only application. The runner validates its
named PHP application before ingress starts.

For custom dispatch, `register_router_with(state, |owner| service)` accepts a
cloneable Tower service over `AppRequest` and adds no fallback. The `Owner`
handle can be used with `.fallback_service(owner)`, mounted on a subtree, or called
explicitly. Its adapter is resolved during boot; neither registration API boots
or resolves PHP while the callback is running. Both APIs use the same ingress
admission and body guards.

```yaml
# lumenize.yaml, loaded from the application's working directory
name: web
fleet_name: web
tcp: 127.0.0.1:8200
orbit: true
php:
  application:
    script_path: application.php
    env_file: .env
logging:
  path: storage/logs/system.log
```

`php.application` declares its persistent PHP script, environment, pool and
execution policy, without repeating the application name. `Runner::new("web")`
already supplies that name, so `name` above is optional. `php.engine`,
`php.lifecycle` and optional `php.sapi` use the existing embed schema. A single
persistent application does not also create a script pool unless requested.
Paths in the file are
relative to the application's working directory. Laravel routes and business
logic remain PHP-owned. Shared Laravel integration remains in Claviron.

Application management reports PHP runtime, Lumenize runner and application
request mode. `frameworks: [Laravel]` in the launch (or repeatable `--framework`)
declares free-form informational tags; an explicit
`php.application.frameworks` takes precedence. Scalar `framework` remains accepted.
When no list is supplied, Composer detection supplies all recognized frameworks;
an explicit `[]` disables discovery. Framework labels do
not select execution or ingress. The same introduction appears through local
management and Orbit Link.

`icon: laravel` (or `--icon laravel`) selects a symbolic admin-GUI icon.
Code uses the same `Options.runner.icon` and `Options.runner.frameworks` fields.
The GUI owns the icon catalog and uses the runtime default for unknown codes.
Neither icons nor framework tags select PHP mode.

PHP configuration documents returned as arrays are supported automatically:
declare `Format::Custom("php")` on a Rust `Configurable` provider. The runner
registers the native Laravel config evaluator before preparation; the Composer
package `claviron/laravel` supplies its helper bootstrap. This evaluates trusted
PHP into the provider's Rust type, without booting Laravel's HTTP application.
`config_parser: {max_depth: 64}` controls the returned array nesting limit.

Cache composition also lives in this file; there is no separate `cache.yaml`:

```yaml
cache:
  default: disk
  stores:
    disk:
      driver: file
      path: storage/cache/runtime
```

Omitting `cache` (or using `cache: {}`) declares no stores. Cache and other launch
policy changes require a process restart.

## Native database pools

Enable `postgres`, `mysql` or `sqlite` for runner-owned Rust pools. The
`database` section in the same launch file is Claviron's existing `DbFileConfig`,
not a Laravel-specific schema or a separate `db.yaml`:

```yaml
database:
  postgres:
    default:
      url: ${DATABASE_URL}
      pool:
        pool_size: 8
        idle_size: 0
        acquire_timeout: 10s
        idle_timeout: 5m
        max_lifetime: 30m
```

Those example capacities and durations are deployment choices. Each process
resolves the section against its boot environment and constructs its own pools
before PHP boots; they are not the edge's pools and SQL is not sent over Orbit.
For PostgreSQL/MySQL, Laravel's existing `'claviron' => 'default'` connection
setting selects the named native pool. SQLite supplies native Rust pools; it
does not introduce a PHP SQLite bridge. A written backend requires its matching
feature. Omission creates no native connections and preserves ordinary PDO
profiles. Runtime pool options change on process restart.

The optional `cli` Cargo feature exposes `Options::from_args()`: `--config-path`
(alias `--config`), `--name`, `--fleet-name`, `--tcp`, `--uds`, `--orbit` and
`--max-in-flight`.
CLI flags override or enable entries in the selected file; use a separate
configuration file to disable an ingress. No listener is enabled implicitly.
The instance name and process-family fleet name are independent. Omitted
`fleet_name` uses the instance name only if it fits the fleet primitive;
longer names need an explicit fleet name. Workers inherit the selected launch.

## Log channels

System logging needs no channel: `logging: {path: storage/logs/system.log}`
selects one file. Enable `log` for named application channels in the same launch:

```yaml
logging:
  system_log:
    path: storage/logs/system.log
  channels:
    laravel:
      adapter: file
      path: storage/logs/system.log
      format: line
```

Each worker copies a record into its lane of a shared-memory ring and returns;
the master's one writer formats and writes it. Laravel reaches a channel through
the `claviron` log driver that `claviron/laravel` registers (a Monolog handler
from `claviron/monolog`). Select it for the runtime only, by setting
`LOG_CHANNEL: claviron` in `php.application.env`; `php artisan`
from a shell keeps the channel `.env` names. Omitted channels create no
application sinks. This runner creates no Edge access records.

## Development source watching

Enable `watch` and `cli` in the application's Lumenize dependency, then start the
same application binary with `--watch`. For example:

```sh
cargo run --features lumenize/watch,lumenize/cli -- --config-path lumenize.dev.yaml --watch
```

A development launch can select source watching in the same file:

```yaml
name: web
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
under `php.engine` in the development launch. Worker replacement alone does not invalidate
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
an edge dependency. For directories under `public`, use the shorthand in the same
`lumenize.yaml`:

```yaml
try_files: false                 # default; no general filesystem probe
assets: [build, images]          # /build → public/build, /images → public/images
```

Each entry becomes Claviron's existing `StaticConfig`, not a separate asset
service or configuration file. `--asset` is repeatable and replaces the written
list. `--try-files` enables general public-file probing; missing files still
reach the application. No directory index is enabled.

For custom roots, prefixes or caching policies, declare explicit mounts instead:

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
Claviron's shared file cache configuration and reload behavior remain separate
from these mounts; this runner does not invent a file cache policy.
`try_files` defaults to false. When enabled, general probing happens after the
explicit mounts. If both forms are used, assets are relative to `files.root`
and an explicit mount at the same prefix retains its policy.

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
- Standalone and supervised master/worker composition use the shared lifecycle.
  Script-mode routing, an application-owned multihost edge and starter generation
  remain future work, independent of the existing PHP pool capacity.

See [ARCHITECTURE.md](ARCHITECTURE.md) for ownership and lifecycle contracts.

## Native Laravel broadcasting

The optional `broadcast` Cargo feature composes Claviron's existing Broadcast service
and Laravel PHP extension in the application process. Declare credentials in the
same `lumenize.yaml`:

```yaml
broadcasting:
  key: ${PUSHER_APP_KEY}
  secret: ${PUSHER_APP_SECRET}
  client_events: true
```

The application name comes from `Runner::new(...)`. The wire `id` defaults to
that name and can be set explicitly for external Pusher REST clients.
The secret must be present in the Rust process environment before bootstrap;
under systemd, supply it with `EnvironmentFile`.

Socket and Orbit ingress serve the configured `/app/{key}` WebSocket endpoints
and signed `/apps/{id}/events` REST endpoints. Orbit carries complete messages
over its existing SHM stream with native wakeups; no TCP or Unix socket listener
is needed in the application. The edge owns the public WebSocket handshake and
wire framing. Both ingress adapters run the same Claviron Broadcast session under
their own task ownership. Laravel's `claviron` broadcast driver calls
the native extension with its selected connection's key. Broadcast resolves that
key against its existing application catalog, with no HTTP client or separate
Pusher process. Laravel's default connection selection is unchanged. Deploy the
matching PHP package and rebuilt host together when updating this native boundary.

## Non-Affiliation

This independent project is not affiliated with or endorsed by Laravel,
Lumen, or the official Illuminate components. It is not a Laravel replacement.

## License

MIT.
