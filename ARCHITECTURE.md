# Lumenize

## Role

Lumenize is a Laravel-focused PHP runner built on Claviron. It is an
independent library project, not the end user's application binary.

The downstream application owns its Rust `main.rs`, binary, Laravel app,
configuration, and application-specific providers. A starter kit may generate
that composition in the future.

## Boundaries

- Consume Claviron through its public root facade, not internal crate paths.
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

The initial `Runner` API composes Claviron's standalone `Server`, one named
persistent PHP application and TCP/Unix-socket and/or Orbit ingress. `Options`
loads `lumenize.yaml`; the optional `cli` feature adds application-owned argv
parsing. No edge preset or additional language engine is selected.

The runner loads `php.yaml`; the application's registration callback owns Rust
providers and services and calls `lumenize::register_router(state, routes::router)`
with a `FnOnce(AppState) -> Router` builder. The registered service uses PHP as
the fallback for undeclared paths and methods. `register_router_with` instead
hands the builder a lazy `PhpService` and registers its Tower service unchanged;
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
composition and supervised process mode remain planned.

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

Process identity, Laravel profile identity and Orbit fleet identity have
different owners. Fleet validation and transport protocol changes must land in
the common runtime before a downstream launch depends on them. Do not assign
new ring kinds or copy the transport into the runner to create isolation.

## Ingress and shutdown

`SocketIngress` binds all selected sockets before accepting. Existing Unix
socket paths cause bind failure; cleanup removes only the inode this generation
created. H1/H2 share one handler. The optional `tls` feature wraps the TCP listener
with one certificate and optional client-CA verification. Material is loaded
before binding sockets; renewal requires restart. ALPN advertises H2/H1.
Handshakes run in the same owned JoinSet, observe listener cancellation, and
introduce no timeout. Verified TLS facts and the physical HTTPS scheme travel
in request context. Unix sockets and Orbit retain their own transport semantics.
Shutdown stops accepts, asks each connection to
shut down gracefully, and joins accepted connections without adding a deadline.
A local cancellation token also ends connections when an accept fails.

Native sockets also own a Claviron `SessionOwner` and connection gate. Hyper's
upgrade-capable connection driver passes the session capability and physical
connection context to dispatch; a Unix socket has unspecified IP addresses.
On shutdown the listener seals session submissions, closes gate admission,
drains HTTP connections, and joins the owned upgraded sessions. No session task
is detached and no shutdown deadline is introduced.

`Application` refuses ambiguous framing and unsupported upgrades, checks declared
request length, wraps incoming bodies in `GuardedBody`, and wraps outgoing bodies
in `guard_reply`. Policies come from `Options`; PHP execution, admission and
recycling policy remain in `php.yaml`. Socket metadata reflects the physical
connection. Orbit reconstructs the original edge peer/scheme facts.

Claviron's `Admission` wraps the common dispatch, including direct TCP/UDS:
`max_concurrent_requests` bounds requests until their response head,
`requests.queue_timeout` bounds waiting for a place, and `head_within` applies
`requests.head_timeout` to service readiness and execution. Queue expiry returns
503; late heads return 504. No value is invented when a limit is omitted.
The admission place ends at the response head, independent of body streaming.

`OrbitIngress` joins the named transport fleet as runtime `php`. A native-wait
reader hands frames into a single-entry channel, while `max_in_flight` bounds
accepted exchange tasks. Waiting work stays in the visible inbox. Shutdown
releases the lane first, then signals and nudges the reader, drains the handoff,
joins the reader and completes accepted exchanges. This follows the common
transport ownership contract; it does not clear shared-memory segments.

Request reads use declared Content-Length, clamped to one transport window;
unknown lengths use the window. A complete response whose exact length fits
one window is written and closed before publishing its head. Larger or unknown
bodies publish their head first, then stream under backpressure. Both use the
same body ticket protocol. Body errors reset the outgoing stream.

The transport receives request bodies incrementally, but the current shared PHP
application service collects them before delivering a request to Laravel. This
is not ASGI-style incremental request delivery into PHP. Synchronous PHP does
not itself require buffering; changing that boundary belongs to the shared PHP
application protocol. Response behavior follows the shared PHP response API.

## Optional Broadcast service

The `broadcast` feature selects the existing Claviron Laravel/Broadcast composition,
without selecting the edge preset. Claviron registers Broadcast from `broadcast.yaml`.
When `broadcast_application` names an application, the runner registers Claviron's
native Laravel PHP extension for it before PHP boots. That extension resolves
the same Broadcast service used by the WebSocket endpoints; Laravel broadcasts
and notifications publish locally through the native function.

During boot, application dispatch wraps the user service with Broadcast's signed
REST publication routes and the shared HTTP/1 upgrade middleware. Configured
app-key paths are exact matches. Native GET/H1 upgrades require listener session
ownership and use Broadcast's fastwebsockets adapter. Orbit ingress validates the
upgrade request and app key, admits a Broadcast generation permit, then publishes
101 and runs its Orbit message adapter inside the ingress JoinSet. Rejections
close the reply direction before publishing their HTTP status; no Broadcast feature
or unknown app key returns 404. No session task is detached.

Broadcast owns the shared message loop, channel authorization, session cancellation
and fleet fanout. Lumenize supplies ingress ownership and route composition.
The separate edge owns public hosts, TLS, the WebSocket handshake and wire
framing. Orbit uses the existing duplex slot and message records, with native
wakeups and backpressure; it requires no application TCP/UDS listener.

## Optional application assets

The `files` feature selects `claviron/files`, independently of `edge`. The facade
registers the existing file-cache provider in its server lifecycle. Lumenize's
`Files` configuration implements `FileSite` for one application and supplies
explicit `statics` mounts to the shared middleware; no host registry is involved.
File dispatch surrounds the Rust/PHP fallback and is inside Broadcast endpoints
and the existing admission/body guards. It applies to all ingress paths. Only
GET/HEAD probe files; HEAD retains headers and omits the body. Mount matching,
file confinement, dynamic-extension protection, caching and conditional responses
remain owned by Claviron. Unmatched paths continue to the application's service.

## Feature ownership and development watch

The default `axum` feature owns `register_router` and its router/body adapter.
The raw `register_router_with` API and PHP dispatch also build without Axum;
Claviron's `http` facade exposes transport primitives separately. `files` and
`broadcast` select Axum for their existing middleware. `cli`, `tls`, `watch`,
`metrics` and `legacy-linux` declare their own dependencies. No feature selects
`edge`; a base build selects neither Axum nor the filesystem watcher.

`watch` registers a source-watching provider only when explicitly enabled in
options. Native notifications retain one pending wake, ignore read events, and
propagate watcher errors. Paths belong to the application; no timer is invented.
The provider subscribes to application-start events before checking status and
uses the process-local `ApplicationManager` to restart only its named PHP
application. PHP owns generation drain, retry and startup. Watch owns its native
subscription, cancels on runtime shutdown and awaits callback release; finalization
also closes a subscription created during boot if no runnable started.

Watch requires development PHP settings with OPcache disabled; it does not
pretend that restarting a pool clears shared bytecode. Runtime configuration and
environment remain sealed, and source watching does not rebuild Rust or restart
listeners/Broadcast. Shared-filesystem notifications are platform dependent and
there is no polling fallback.
