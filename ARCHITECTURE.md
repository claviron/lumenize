# Lumenize

## Role

Lumenize is a Laravel-focused PHP runner built on Claviron. It is an
independent library project, not the end user's application binary.

The downstream application owns its Rust `main.rs`, binary, Laravel app,
configuration, and application-specific providers. A starter kit may generate
that composition in the future.

## Boundaries

- Consume the runner shell directly through `claviron-runner`; select its
  own features. The Claviron facade still supplies server lifecycle, PHP and
  Laravel integration. Neither runner nor facade selects an edge implicitly.
- Compose the existing PHP runner and embedded PHP extension mechanisms;
  do not introduce a second PHP engine or supervisor.
- Shared Laravel integration stays in Claviron and remains usable without
  Lumenize. Only runner-specific composition and future starter tooling belong
  here; application-specific host extensions belong to the downstream app.
- Edge, upstream transport, and additional language runtimes are optional
  composition choices, not mandatory dependencies of every application.
- Providers register definitions without resolving dependencies during
  registration. Resolve dependencies in boot; long-running services implement
  the runtime's Runnable contract and observe its shutdown signal.
- Worker supervision and fleet-scoped Orbit state use Claviron's existing
  contracts. Do not invent a separate process or shared-state lifecycle.

## Current state

The fluent builder accepts `with_config_path`, `with_fleet_name`, typed
`with_services::<(ServiceA, ServiceB)>()`, and `with_bootstrap` callbacks.
The Axum surface exposes Claviron HTTP's `Routable`. Its
`routes(self: Arc<Self>, &AppState)` receives the registered shared instance,
so application handlers may capture it without a second registry lookup or
an Axum state/extension just to access their provider. Registration still
passes an Arc and the result remains `Option<Router<AppState>>`.
Services implement Continuo's `Register<AppState>`, re-exported through Claviron
runtime and Lumenize; Lumenize does not invent
a lifecycle trait or construct them with an implicit `Default` requirement.
Registration callbacks run in builder order before boot; `run()` starts the
composition. `with_interceptor` builds its selected router after
all application registrations, so registered Routable contributions are visible
regardless of builder call order. A subsequent interceptor replaces the prior
selection; it does not register a duplicate application service.

The initial `Runner` API composes Claviron's standalone `Server`, one named
persistent PHP application and TCP/Unix-socket and/or Orbit ingress. `Options`
loads `lumenize.yaml`; the optional `cli` feature adds application-owned argv
parsing. No edge preset or additional language engine is selected.

The runner loads one launch document; the application's registration callback owns Rust
providers and services and calls `lumenize::register_router(state, routes::router)`
with a `FnOnce(AppState) -> Router` builder. The registered service uses PHP as
the fallback for undeclared paths and methods. `register_router_with` instead
hands the builder the shell's `Owner` and registers its Tower service unchanged;
the application decides where PHP belongs. Both registrations use one provider
type, so duplicate registrations fail. `Application` resolves and binds the
shared PHP adapter in boot and validates the selected PHP name
against the completed catalog before any runnable starts. A matching Rust
handler's response is authoritative, including 404. Request and response guards
surround both Rust and PHP dispatch. Provider boot contributions are supported.
No PHP package migration or starter generator is implemented.

## Downstream repository contract

An application can keep Composer and Cargo in one repository. Its PHP source
layout and autoload mappings are independent of Cargo's target paths: the
application may name a root `main.rs` explicitly and keep `src/` for PHP.
Application domain crates may become members of its own Cargo workspace as
capabilities are implemented. Lumenize remains an external library dependency,
not the parent workspace or the owner of those domain crates.

The first acceptance path uses a real downstream Laravel application with its
existing persistent entry. Private application profiles, credentials, payment
logic and operator configuration never become library fixtures or generated
starter defaults. A future isolated smoke fixture should contain only the
minimum generic Laravel behavior it tests.

## PHP composition contract

The application owns the binary and registration callback. The runner composes
a Claviron server, invokes the callback, and registers its ingress providers.
Claviron owns boot, admission, runnable generations, drain and finalize. The
runner must not introduce an independent lifecycle or supervise PHP itself.

Persistent Laravel execution uses the shared PHP application receive/respond
loop and Octane operation isolation. The shared runtime supports script execution;
Lumenize ingress currently targets only one persistent application. Script
routing is a later composition. Each persistent context has a fixed profile
environment, established before Laravel loads cached configuration; no request
changes the process environment to select another application.

PHP execution capacity and process supervision are separate choices. A Python
interpreter's virtual-environment restrictions are not a PHP deployment policy.

## Serving compositions

The roadmap has four deployment shapes around the same PHP/Laravel
execution boundary. These describe responsibilities, not four separate engines
or a commitment to a mutually exclusive configuration enum.

| Composition | Request ingress | Vhost owner |
| --- | --- | --- |
| Development server | Application-local HTTP listener for development | No multi-vhost edge configuration |
| Orbit application | A separate edge sends exchanges over the shared-memory transport | The separate edge |
| Socket upstream application | A separate edge proxies HTTP over TCP or a Unix socket | The separate edge |
| Embedded edge | The application binary composes Claviron's listeners, TLS, virtual hosts, routes and static files | The application binary |

Serving as an upstream is the application's ingress role. It is distinct from
enabling Claviron's reverse-upstream capability to proxy an outgoing route to
another application. Orbit streams provide the bounded body transport; the
shared exchange protocol also carries addressing and request/response metadata.
The runner must use that protocol rather than create a PHP-specific wire format.

The initial implementation exercises a PHP application behind a separate edge.
An application-owned multi-vhost edge remains a downstream requirement. Vhost and
edge routing policy belong only to the edge composition. An application served
behind another edge does not inherit that edge's vhost configuration.

These serving shapes are independent of standalone/master/worker process roles,
supervisor policy, Laravel profile selection, and Cargo debug/release profiles.
Selecting an ingress does not create a second process lifecycle. The development,
socket-upstream and Orbit compositions are implemented. The embedded edge
composition remains planned. Supervised master/worker mode uses the shared
supervisor and is selected by `workers`.

## Shared foundations

- Continuo owns the typed process-local registry and live runnable generations.
  Providers resolve the same registered instances, declare lifecycle ordering,
  and drain their accepted work before reverse-order finalization. Lumenize
  reaches this contract through Claviron's runtime facade.
- Orbitive supplies bounded same-host memory primitives: history rings,
  current-state tables and duplex streams. It does not own application boot,
  language execution, supervision or liveness policy. Claviron's Orbit layer
  connects these primitives to the runtime and the application transport.
- Claviron's `runtime`, `server`, `supervisor` and `edge` capabilities are
  distinct composition levels. Use the level each serving shape needs rather
  than forcing every runner to boot edge services.
- Cross-host federation is future work in Orbitive's vision, not an existing
  property of the same-host transport. Multiple HTTP virtual hosts in an edge
  application do not imply cross-machine shared memory.

## Configuration boundary

`lumenize.yaml` is the typed composition entry. Reuse the runtime's
public configuration types and loading contracts. Laravel's PHP configuration
continues to own routes, middleware, authentication and business settings;
booting Laravel to discover Rust boot dependencies would reverse that order.
PHP YAML parsing and environment resolution stay in Claviron; Lumenize does not
maintain a second PHP schema.

`Options.config_path` selects one file (default `lumenize.yaml`); there is no
public `config_dir` option. The flat common runner keys sit beside `php`,
`broadcasting`, `logging`, `cache`, and `config_parser`. They include
`assets: [build, images]` and opt-in `try_files` (default false).
With the `files` feature, these use the shell's existing static mounts and
public-file probe; they create no separate asset configuration or service.
`php.application` supplies one application whose
name is `Runner::new(...)`, without a repeated application catalog key.
The existing embed schema validates that section. `broadcasting` is the existing
single application's catalog entry, with its ID defaulting to the application
name; its key and secret remain explicit. `logging.path` is a single system-log
shorthand, or `logging.system_log` and `logging.channels` use the existing
logging policies. There is no Edge access-log registration here.

The runner always installs `LaravelConfigParser` on the shared ConfigReader
after declaring the process PHP INI and before any configuration preparation.
Owners can use `Format::Custom("php")` for returned PHP arrays, including
Laravel helper calls, with no application-local parser registration. The package
runner at `vendor/claviron/laravel/runners/config.php` uses the consumer's Composer
autoload; evaluation does not boot its HTTP loop or Laravel service providers.
`config_parser.max_depth` bounds returned array nesting (default 64).

`cache` reuses Claviron's `CacheConfig` store/driver schema. Omission or an empty
section creates no stores, without consulting `cache.yaml`. The cache registers
before default composition and builds after all custom driver registrations,
before cache-dependent services boot. It is immutable launch intent like PHP;
changes require process restart.

Explicit definitions are installed through `Server::with_definitions` before
built-in defaults; those defaults retain the supplied PHP runtime, broadcast
catalog and log channels without opening separate files. Ordinary application
registrations still run after built-in registration. Runtime sections are
immutable boot intent and require restart after changes. Source watching remains
the separate PHP-source restart API.

The launch's supervisor policy is also installed in this definitions phase,
before Claviron constructs its supervisor consumers. An omitted `supervisor`
section uses the supervisor's existing defaults. Installing that source later
would disable file preparation without initializing the consumers' supplied
snapshots; supervised launches must never enter that state.

The CLI's `--config-path` selects the file and its runner flags override written
values. `Runner::new(...)` supplies the default instance name; `fleet_name` and
`--fleet-name` independently select the process-family namespace. A worker
inherits the launch and file identity rather than reopening a default file.
Unresolved environment expressions travel unchanged and use each process's
prepared boot environment; resolved secrets are not copied into the launch.

Process identity, Laravel profile identity and Orbit fleet identity have
different owners. Fleet validation and transport protocol changes must land in
the common runtime before a downstream launch depends on them. Do not assign
new ring kinds or copy the transport into the runner to create isolation.

## The runner shell

Everything a runner does alike is Claviron's runner shell (`claviron_runner`,
`crates/runner` in Claviron): the TCP/Unix socket ingress (HTTP/1 and HTTP/2,
TLS, upgrades, trusted forwarding headers), the Orbit ingress, the guards and
admission, the routing order, static mounts, broadcasting, source watching,
the launch's shared options and the workers. Its own `ARCHITECTURE.md` is
where they are described; the same code serves the Python runner.

What Lumenize adds is the owner and what is Laravel's:

- **The owner** is the launch's PHP application, bound when PHP boots
  (`PhpOwner`); it answers what the routes, files and broadcasting in front
  of it did not claim. Its Orbit lane says `php`.
- **Options** are one flat `lumenize.yaml`: the runner's shared keys beside
  Lumenize's own `php`, `broadcasting` and `logging` sections.
- **Broadcasting** from PHP: `broadcasting` registers Claviron's
  native Laravel publisher; the Pusher endpoints themselves are the shell's.
- **Watching** takes Laravel's source directories and restarts the PHP
  application in its process once it has started. It refuses workers: with
  them each worker has its own application and the watching master has none.
- **Workers** come from the shell with `workers:`; each worker boots its own
  PHP and takes its own lane, and receives the master's resolved launch, since
  it starts with no command line.
