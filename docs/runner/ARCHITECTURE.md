# Lumenize Runner

## Role

The root `lumenize` library's `runner` feature composes a Laravel-focused PHP
application on Claviron. It is optional, not the library's defining dependency.
The separate `lumenize-cli` package is only an Artisan console executable.

The downstream application owns its Rust `main.rs`, binary, Laravel app,
configuration, and application-specific providers. A starter kit may generate
that composition in the future.

## Boundaries

- Consume the runner shell through the public `claviron::runner` facade.
  Claviron supplies lifecycle and the PHP engine; the root `lumenize` library
  supplies Laravel integration. Neither runner nor facade selects an edge implicitly.
- Compose the existing PHP runner and embedded PHP extension mechanisms;
  do not introduce a second PHP engine or supervisor.
- Shared Laravel integration remains usable without the `runner` feature.
  Runner modules, options and their tests live in the root package; the
  executable package depends only on its independent `cli` entry.
  Application-specific host extensions belong to the downstream app.
- Edge, upstream transport, and additional language runtimes are optional
  composition choices, not mandatory dependencies of every application.
- Providers register definitions without resolving dependencies during
  registration. Resolve dependencies in boot; long-running services implement
  the runtime's Runnable contract and observe its shutdown signal.
- Worker supervision and fleet-scoped Orbit state use Claviron's existing
  contracts. Do not invent a separate process or shared-state lifecycle.

## Current state

The hosted Application reports PHP runtime, Lumenize runner and the selected
request mode through Claviron's common description. Frameworks are optional,
free-form informational tags declared by `frameworks` in the launch/CLI or an
explicit `php.application.frameworks`; an absent list is detected through
Composer when recognized. An explicit empty list suppresses detection; scalar
`framework` remains a single-label shorthand. Optional `icon` is a client-owned
presentation code, not an execution selector. An omitted icon resolves to
`laravel` before launch serialization; explicit codes are preserved.

`request_mode` selects the actual PHP adapter, not just its label.
Without a selection, Composer's `laravel/octane` selects application mode and
its absence selects script mode. `with_request_mode` overrides YAML independent
of builder order. Resolution happens before launch serialization; workers
inherit the choice. Installed Composer records take precedence over the lockfile
and root runtime requirements. No HTTP request performs detection.
TCP/UDS/Orbit are ingress methods, independent of this introduction.

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
PHP application in either request mode and TCP/Unix-socket and/or Orbit ingress. `Options`
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
loop and Octane operation isolation. Script execution uses the existing SAPI
request pool and `PhpScriptAdapter`, with `public/index.php` as its default
front controller. `php.script` reuses the public `PhpConfig` route type;
the optional `php.sapi` uses the same engine pool schema as the edge.
Each persistent context has a fixed profile
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

Orbit launches obtain Link connection observations and hosted-application
management from the shell's `launch::register_doors`; Lumenize registers no
duplicate command bridge. The shared PHP engine's `ApplicationController`
remains authoritative for start/stop/restart and lifecycle snapshots. Stopping
the PHP application does not stop its runner or disconnect its Link lane.

### Benchmark execution identity

The opt-in PHP `Http\Controllers\BenchController` consumes the existing Octane
client's request context. It emits that context's boot ID and increasing request
count and fails when persistent-client context is absent. It owns no native
service, session or database access. Applications register its actions.
