# Laravel Integration Boundary

## Repository and boot ownership

`examples/laravel` is an unpublished workspace member for local co-development.
Its Rust host consumes the root library through a workspace path dependency;
Composer symlinks the local `php/` package. Laravel owns a process-local PDO
SQLite connection and migrations. The default ingress is loopback TCP, not
Orbit; optional Edge composition is documented separately. This application
is not a library fixture and is never booted by the library's Cargo tests.

The root `lumenize` package owns the native Laravel adapters and the Composer
package under `php/`. It consumes only Claviron's public facade. Configuration
and Artisan APIs are available without a serving composition; independent
`auth` and `broadcast` features expose those integration modules. Importing or
registering them starts no server and creates no second AppState, PHP engine,
supervisor or listener. The host supplies existing state and services.

The root library's opt-in `runner` feature owns Runner, launch/options, PHP
owner and watcher composition. Its independent `cli` feature exposes Artisan
execution from process arguments and adds launch parsing when `runner` is also
selected. Neither is a default feature. Integration-only hosts register adapters
during their own boot without enabling Runner, server or Supervisor.
See [Runner architecture](docs/runner/ARCHITECTURE.md) for the serving contract.

`cli/` is the separate `lumenize-cli` executable package. Its thin binary
calls `cli::run()` with the `cli` feature, not `runner`. It depends on this
library; the reverse dependency is forbidden. Core Claviron does not depend on
either package.

PHP integration classes use `Lumenize\`; the engine's `Claviron\` classes
remain engine-owned. Composer identity `claviron/laravel`, native function names,
driver names and event topics are unchanged. Laravel admission, guard,
notification and financial semantics do not change.

## Laravel service providers

The Composer package's service providers live under `Lumenize\Providers`.
Their short class names are `AuthServiceProvider`, `BroadcastingProvider`,
`CacheServiceProvider`, `DatabaseServiceProvider`, `LogServiceProvider`,
`QueueServiceProvider`, `ScheduleServiceProvider` and `SessionServiceProvider`.
`LumenizeProvider` supplies console-only configuration defaults and a config
publishing group. The namespace carries the package identity; class names do not repeat it.
Applications can import `Lumenize\Providers` and register
`Providers\CacheServiceProvider::class` without colliding with their own
application provider names. Existing driver names and registration order are
unchanged. There are no old-name aliases: upgrade explicit registrations and
Composer discovery metadata, and regenerate provider/config caches retaining
old class names before restarting the consuming PHP instances.
Drain or explicitly migrate serialized queue/invocation payloads containing
the former integration class names before switching workers; no alias or live
payload rewrite is introduced. Native envelope classes and topic strings are
not renamed by this PHP namespace change.

## Laravel-authored documents read by Claviron

`congif::LaravelConfigParser` implements the external Congif parser contract
for `.php` documents. These are Claviron inputs written with Laravel helpers,
**not Laravel's own config repository**. Laravel keeps loading its config as usual.
Register the parser on a ConfigReader and declare `Format::Custom("php")` on
the consuming owner; Congif owns file reads, typed decode, validation, equality,
publication and source watching, exactly as with YAML/JSON.

`congif::LaravelConfigProvider` implements `Register<AppState>` for the standard
host composition. `LaravelConfigProvider::register(state)` takes the application
root from the process's current working directory and registers the parser with
the existing 64-level returned-array nesting bound. It is registration-only:
no runnable or empty lifecycle instance is inserted, and no PHP is evaluated.
Register it before configuration preparation, after declaring process INI.
The parser remains independently constructible for custom runner/evaluator or
nesting policies; it does not implement the runtime Provider trait.

The Composer package ships `runners/config.php`. It loads the consumer's
autoload and prepares a minimal Illuminate Application/helper container, then
the native evaluator executes Congif's pinned source bytes with the original
filename. It does not load `bootstrap/app.php`, Laravel configuration files,
service providers, a database or the Octane loop. `Str`, `env`, `app` and
`base_path` are available; a facade requiring a booted provider is not promised.
No extra runner is copied/published into the application root.

`runners/application.php` is the optional package-shipped Octane HTTP entry.
It loads the consumer's autoloader from its first argument and enters
`Octane\ApplicationRunner`; it neither evaluates configuration nor runs Artisan.
Lumenize can use it when application mode has no explicit entry and no local
`application.php`. Script mode retains the consumer's normal `public/index.php`.
The host pins the project root in that argument before launch serialization,
so PHP changing its virtual cwd to the package entry cannot change the root.

Congif's variables enter `env()` through the pinned `$_ENV`/`$_SERVER` maps,
with Laravel's ordinary boolean/null/default interpretation. PHP source is
never text-interpolated. A new evaluation has a fresh Zend scope; only owned
Rust values leave it, with no JSON round-trip. PHP lists become sequences and
associative/sparse arrays become maps; empty arrays are sequences. Objects,
resources, recursive arrays and values beyond the caller's declared nesting
limit fail. These are trusted executable documents, not a security sandbox.

Evaluation may run during preparation before any provider's `configure`/`boot`.
Declare the one process's PHP INI options and all host extensions during cold
composition before the first evaluation. It shares PHP's native SAPI with
hosted execution rather than booting a second engine. No HTTP runtime or
AppState is required by the evaluation API. The helper-only regression takes
an installed autoloader: `php php/tests/config-evaluator.php /path/to/vendor/autoload.php`.
Native engine execution belongs in an external executable harness, not this
library's Cargo test suite.

## Lumenize console executable

`cli/src/bin/lumenize.rs` calls the root library's `cli::run()` entry. Its
default behavior is to launch local Artisan commands. It uses
`php-embed::console::PhpConsole`, not the facade's server feature, an HTTP
listener, TaskService or an invocation command. Run it from the Laravel
application root with ordinary Artisan arguments. It executes the application's
own `./artisan` file in the native engine, preserving its bootstrap, custom
preparation and command registration. Rust does not bypass it with a package
runner or duplicate Laravel boot. The application's entry owns autoloading,
dependencies, kernel termination and any application-specific host setup.

Before PHP starts, the entry checks only that `./artisan` is a file. If absent,
it reports that no Laravel application was found here and exits with failure.
No Composer package is required by the launcher or installed automatically.

The binary preserves literal UTF-8 arguments and the caller's environment,
declares `APP_RUNNING_IN_CONSOLE=true` for the non-CLI SAPI, and inherits the
terminal streams. Ctrl-C signals the owned execution's cancellation token;
completion still joins its PHP thread. This is cooperative cancellation, not
permission to kill foreign blocking work. PHP INI selection remains process-owned.

Build/install it with `cargo install --path cli --features cli --bin lumenize`,
or install `lumenize-cli` with its default `cli` feature. The root library
has no binary. Claviron's weak `runner-cli` forwarding adds shared argument
parsing only when the serving runner is separately selected.
The first standalone entry does not load runtime YAML, bind native DB/cache
services or attach to Orbit. Laravel can use its ordinary PHP/PDO drivers;
explicit native driver configuration still fails closed without prepared host
capabilities. `--live` is not implemented and no implicit fleet is joined.
Compiled framework-neutral host functions remain available through the SAPI;
runtime-backed operations do not become ready merely because their functions exist.

## Console configuration declaration

`Providers\LumenizeProvider` is included in Composer discovery. Its registration
and boot methods are no-ops outside console execution, including HTTP under
the embedded SAPI. Console registration merges the package's `config/lumenize.php`
into Laravel's `lumenize` repository key; application values take precedence and
Laravel's configuration cache remains authoritative. The default `config_path`
is `lumenize.yaml`, a document path relative to the application's working
directory. It is not resolved against the package or compiled binary location.

Console boot exposes `lumenize-config` for publishing to the application's
`config/lumenize.php`. Consumers disabling package discovery explicitly register
the provider. No file needs to be published to use defaults. This is a declaration
boundary only: it does not read YAML, prepare DB/cache services, subscribe to
events or join Orbit. Runtime preparation and project-owned binary composition
remain separate future work. The isolated check is
`php php/tests/lumenize-provider.php /path/to/vendor/autoload.php`.

## Native request authentication

`Providers\AuthServiceProvider` registers the opt-in `claviron` auth guard.
The application selects a Laravel user provider implementing `NativeUserProvider`.
`Auth\\FleetSession` hydrates its `Authenticatable` from `claviron_auth()`; Laravel retains
authorization/Gates and model hydration. PHP neither validates credentials nor
calls the session server over HTTP. Missing native support fails closed.

The host binds `php-embed::auth::PhpRequestAuthenticator`. It owns credential
extraction, audience/purpose, lazy user lookup and session activity policy.
Rust loads the user once, or reuses an `AuthenticatedSession` already attached
to this request. The provided `Auth\\EloquentUserProvider` hydrates the configured
application model using `newFromBuilder`: casts, hidden fields, originals and
model events retain Eloquent semantics, without another query. Applications
can extend this provider or implement the hydration contract themselves.
`subject_key` maps the verified subject to a raw attribute; optional
application-owned `constraints` (for example active status) are checked locally.
Without `subject_key`, the hydrated model's auth identifier is checked. A
provider without the native hydration contract fails rather than doing a second
lookup. The native function returns subject, optional session_id, credential
expires_at and raw model attributes, never a raw token or mutable session.
Attributes are internal hydration data, not a public response serializer.
Script Parts and
application Jobs own the cached verified result; task/event/idle executions
return null. Live revocation and expiry are rechecked on every native read.
There is no process-wide PHP token-to-user cache.
`FleetSessionGuard::setRequest` also forgets Laravel's hydrated user when its
container receives another request. The isolated PHP adapter regression is
`php php/tests/fleet-session.php /path/to/vendor/autoload.php`; it boots no
application and substitutes only the native identity function.

## Native broadcasting

`broadcast` replaces Laravel's server-side Pusher HTTP publication with
one compiled-in PHP function:

```php
claviron_laravel_broadcast($key, $channels, $event, $jsonData, $socketId);
```

Rust's `broadcast` module owns the extension, registration, native handler and
publication context. `broadcast::LaravelBroadcastProvider` implements
`Register<AppState>` and installs `broadcast::BroadcastExtension` on serving
processes without a fixed application name. The crate root declares modules
without generic extension or registration aliases. This extension is named
`claviron-laravel-broadcast` and registers only `claviron_laravel_broadcast`.
Authentication, configuration evaluation and console execution remain separate
integration surfaces.

The Rust extension resolves the worker-local `BroadcastService` during Claviron
registration. PHP supplies the key from its selected Laravel broadcast connection;
Laravel's ordinary default/explicit connection selection remains unchanged.
`BroadcastService::publish_by_key` resolves the application in the existing
catalog, rejecting unknown keys without fallback or logging the key. The native
extension retains only the service, not an application identity or another catalog.
It is a trusted in-process publisher, not the external REST authentication API.
A call validates the Pusher limits and publishes directly into
Broadcast's dedicated multi-chunk Orbit ring. It performs no DNS lookup, TLS
handshake, HTTP request, or loopback socket connection.

The PHP package registers a normal Laravel broadcast driver named `claviron`.
It retains Laravel's channel authorization contract and produces
Pusher-compatible private and presence HMAC responses for `/broadcasting/auth`.
Only the server-side `broadcast()` transport changes. Echo and PusherJS clients
continue to use the standard Pusher WebSocket protocol.
When the key or secret is missing, driver resolution still permits channel
registration during Composer discovery, but does not construct the credentialed
broadcaster. Authentication and publication then fail with a configuration error.

The native function's key argument is required. Deploy the matching PHP package
and rebuilt host together and restart PHP contexts; there is no old-signature
compatibility path that silently targets a default application.

The generic Pusher REST endpoint remains an independent compatibility surface
for external publishers. It is not the native Laravel driver's transport.

## Framework logging adapter

The PHP package's log provider registers the `claviron` Monolog driver on
Laravel's application-owned `LogManager`. Lightweight containers such as
Lumen, which bind that manager only as `LoggerInterface`, call
`Providers\LogServiceProvider::registerDriver()` with their existing singleton;
they do not construct a second manager. Outside the embedded runtime the
application keeps its ordinary file channel and never resolves the native
handler.

## Persistent application entry

`Octane\ApplicationRunner` owns the receive/request/event/terminate loop.
The consumer loads Composer and calls `run(basePath: ..., requestClass: ...)`;
the optional request class extends Illuminate's Request. It may also be a closure
receiving the already-booted Laravel application and returning that class. The
runner resolves and validates it once before admission, inside the worker's
termination boundary. The client constructs the selected class directly;
there is no intermediate Request conversion or second application boot.
Application paths are explicit, never inferred from the package's `__DIR__` or
the process cwd.
The entry may live outside the public document root. Runtime receive ending
or the Octane client stopping ends admission; the worker then terminates.
Worker failures retain their original exception at the native host boundary.
Consumers still own their bootstrap, profiles, providers and authentication.

## Native SQLite database adapter

SQLite profiles with `claviron` use `Database\SQLiteConnection` over a named
Rust pool. Hosted use requires the `sqlite` feature; Artisan continues using
PDO. `SQLiteProcessor` supplies insert IDs from native statement results.

Laravel keeps its query and transaction APIs. Claviron executes SQL in the PHP
process and rolls back unfinished transactions at request boundaries. Pool
configuration controls lock waiting. An uncertain COMMIT outcome or a failed
`afterCommit` callback must not replay the transaction.

## Native MySQL database adapter

`Providers\DatabaseServiceProvider` recognizes `claviron: true` or a named pool
on Laravel `mysql` connections. Ordinary CLI/Artisan uses Laravel's PDO factory;
inside Claviron, opting in requires the native `mysql` feature and fails closed
if it is missing. `Database\MySqlConnection` owns Laravel grammar, query events,
Eloquent hydration, insert IDs, savepoints, transaction callbacks and retry
policy. `php-embed` owns the actual request lease. No copy belongs in a consumer.
The connection's Laravel name is preserved so model writes remain in its
transaction. Ambiguous commit failures are propagated, never automatically
replayed. MySQL errno and SQLSTATE survive the PHP exception boundary.

MySQL and PostgreSQL separate the native COMMIT/retry phase from transaction-manager
callbacks and the `committed` event. A post-commit exception propagates unchanged
without replaying the completed closure or changing a transaction opened by the
callback. Native COMMIT deadlocks retain the existing rollback/retry policy.

When native cleanup has rolled back abandoned work, MySQL/PostgreSQL
`transactionLevel()` clears both the stale nesting count and that connection's
pending/staged transaction-manager callbacks. `beginTransaction()` reconciles
this before registering the next transaction. New callbacks and records owned
by other connections remain intact; reconciliation issues no extra rollback SQL.

Connection/session options come from the named `db.yaml` pool, not Laravel's
PDO options: match charset, collation, timezone and SQL mode before opting a
consumer in. Decimal results remain strings. The first MySQL implementation
buffers `cursor()` like PDO MySQL's default; multiple result sets are rejected.
It closes its session at each work-unit boundary rather than claiming reusable
pool reset, which SQLx currently does not expose. DDL has MySQL's usual implicit
commit semantics and must stay out of cashier transactions.

The consumer-independent adapter test takes an installed Laravel autoloader:
`php php/tests/mysql.php /path/to/vendor/autoload.php`. It substitutes only the
native boundary; it does not replace a real embedded MySQL integration test.

## Native PostgreSQL operation cleanup

`Database\PostgresConnection::disconnect()` releases the thread's native lease
through `claviron_postgres_disconnect`, clears Laravel's nesting count and
discards abandoned transaction callbacks. It retains the parent's no-PDO
sentinel: inherited PDO disconnect/reconnect must not reset the PHP count
after the next handler has opened its native transaction. Native request
cleanup and explicit disconnect are idempotent; neither commits abandoned work.
The next statement leases a connection from the pool without PDO reconnection.

`php php/tests/postgres.php /path/to/vendor/autoload.php` pins the
disconnect → begin → first-query regression using Laravel's real query and
transaction pipelines with a substituted native boundary. It also checks
savepoints, commit callbacks and repeated operation cleanup without a database.

Standalone embedded execution uses the same declared cache fallback and PDO
paths as ordinary PHP. Cache and database adapters check
`claviron_runtime_configured()` before choosing host-owned native services;
the engine-presence marker alone cannot select a prepared host. Once host
bindings are prepared, missing native features or resources still fail explicitly.

## Cache value encoding

`Cache\Store` keeps ordinary scalars and arrays native. Objects and strings
beginning with its reserved object marker use that marker followed by PHP
serialization. A marker-prefixed string is serialized as a string, so decoding
one layer returns its exact bytes without interpreting its contents as objects.
The existing object wire format and existing readers remain compatible.

Previously stored, unescaped marker-prefixed strings are indistinguishable from
serialized envelopes. Expire or rewrite affected keys through the updated codec
when their original literal value is known.

## Cache-backed sessions

The PHP package registers a Laravel session driver named `claviron`. It is a
thin adapter around Laravel's `CacheBasedSessionHandler` and a configured
Claviron cache repository; there is no second session store in this crate.
An explicit `session.store` selects that Laravel cache store. When absent or
empty, the provider lets CacheManager select the configured `cache.default`;
the `claviron` driver name never implies a cache store with that name.

`Http\Middleware\StartNativeSession` is the opt-in native identity binding,
registered as `native.session:<data-namespace>`. It consumes `claviron_auth()`
from this request, never parses credentials or chooses another guard. The host
remains responsible for admission, user policy, expiry and revocation. A live
native session ID and credential expiry are required; anonymous requests fail
unless an application subclass explicitly allows a transient login form.

The namespace and native ID derive Laravel's data ID. It is not another auth
credential and is never recovered from a client cookie. A selected
`session.store` is honored, otherwise data uses `cache.default`. Remaining
native credential life bounds writes, recomputed after the handler. No Laravel
session cookie is emitted and Laravel data-ID regeneration cannot detach the
payload from its native session. Reusable middleware resets its native binding
after each request; each data load clears the previous PHP attributes.

Laravel still owns session serialization and flash aging. The default payload
is PHP serialization, not a cross-language native session map. Applications
may choose Laravel's built-in JSON serialization, but cross-language writers
must also coordinate payload updates and the `_flash` lifecycle; registering
this middleware alone does not provide Rust flash access. Application-specific
CSRF token sources, audit metadata and obsolete-cookie cleanup belong in the
consumer, not this shared binding. The isolated regression is
`php php/tests/native-session.php /path/to/vendor/autoload.php`.

A `claviron` cache store may name another Laravel store as `fallback`. The
provider resolves that store when the PHP process is not attached to a
Claviron runtime, so ordinary Artisan and test processes can retain real cache
semantics without baking runtime detection into cached application config.
Without an explicit fallback the existing detached, fail-closed store remains
the default. A store cannot fall back to itself.

The named cache store determines the runtime behavior. In the full runtime a
dedicated Orbit store can use fleet-local L1 state with a Redis L2, while its
Laravel cache repository also supplies Orbit locks for Laravel's optional
per-session request blocking. Session expiry remains Laravel's ordinary cache
TTL contract, and garbage collection is consequently a no-op.

## Scheduled background commands

The schedule provider keeps Laravel's ordinary `schedule:run` contract but
replaces an opted-in `runInBackground()` subprocess with
`orbit_invoke('claviron.php.program.invoke.v1', ...)` when the primitive is
available. Its payload declares `kind=job`, the absolute Artisan path, literal
arguments and console environment. The returned invocation ID is correlation,
not an acknowledgement that the command ran.

`php-embed` decodes and executes that payload; this crate only maps
Laravel scheduling intent into it. `errand:run` observes the invocation ID plus
submission/admission timestamps, owns the Orbit overlap lock, executes the
command, and performs Laravel's `schedule:finish` callbacks. A publication
failure falls back to Laravel's original subprocess path.

## Event and invocation task consumer

`Link\LinkSubscriber` is the opt-in observer for direct Link connections,
departures and edge shutdown announcements. `Link\Events\EdgeConnected` and
`ApplicationConnected` come from this process's transport directory, once per
observed opposite-side lane. They are local notices, never questions or replies
broadcast to other workers or other linked applications. Fleet names identify
the Link segment; roles identify the transport, and an unknown PID stays null.
The former `TransponderQuery` / `TransponderReply` classes and subscriptions
are removed; consumers use the direct connection classes instead.
Registering the subscriber in the application's
event provider's `$subscribe` also receives
`Link\Events\EdgeShutdownInitiated` and logs the edge, fleet and application.
The announcement is local to the receiving process and does not by itself
stop PHP; the runner owns its separate edge shutdown policy.

`EventBridge::available()` uses the native event-readiness probe, not the
engine-presence marker. Detached console boot therefore does not start event
subscriptions merely because Claviron's functions are registered. A configured
host retains its existing subscriptions; operations requiring that host still
fail explicitly if it is unavailable.

`Invocation\Runner` runs in the existing PHP TaskService execution. Its
`claviron_task_receive()` native wait multiplexes event and invocation input
with cancellation, without the former one-second invocation timeout loop.
`invoke:serve` has no handler, event or budget arguments. The application's
`config/claviron.php` selects `invocations.registrars` and `invocations.budget`
(seconds per operation, zero retains the unbounded policy). Each registrar
implements `Invocation\Registrar::register(Invocation\Registry)` and declares
handlers with `handler(Class::class)` and inbound events with `event(Class::class)`.
Declarations do not instantiate handlers, open subscriptions or start loops.
The catalog validates all classes, rejects conflicting event-topic decoders,
deduplicates repeated declarations and seals registrations before serving.
An incoming operation cannot instantiate a class outside this catalog.

The catalog belongs to one application container. Profile selection belongs
to the application's config, not a library-wide `APP_PROFILE` convention;
select it before framework boot and use that profile's config cache. Only the
serving task subscribes. Loading the provider for HTTP, queue or scheduler
execution does not start an invocation consumer. An empty catalog rejects
serving rather than silently waiting for nothing. Handlers are resolved from
the container per invocation. EventBridge dispatches native events through
Laravel's existing listeners; invocations retain correlated completed/failed outcomes.
Subscriptions are bounded and start at the current head, not a durable queue.
Events have no invocation outcome. A listener may enqueue work using Laravel's
ordinary `ShouldQueue` contract. The queue worker remains a separate consumer.

`Notifications\Request` is a versioned array contract with an application-owned
notification name, recipient reference and data. `Notifications\Handler`
implements the existing invocation interface. The application subclasses it to
resolve recipients, enforce domain policy and construct allowed Notification
objects; wire data never chooses arbitrary PHP classes. Laravel's notification
dispatcher owns `via()`, queued versus synchronous delivery, mail, database and
broadcast channels. Broadcast uses the existing native driver back into Rust,
not HTTP. The result distinguishes `queued` from `dispatched`; neither means
browser or SMTP receipt, and neither exposes a general queue job ID. Errors
propagate into the existing invocation failure outcome. No domain table, user
model, notification registry, new fleet protocol or additional daemon is added.

## Native Laravel auth lifecycle bridge

Rust `auth::register` listens to Claviron Auth's `AuthEvent` only when selected
by the host and converts it to `claviron.auth.event.v1` ApplicationEvent frames.
The host supplies ApplicationEvent fleet transport and one PHP event-consuming
task. This is a bounded event stream, not durable queue admission or replay.
There is no HTTP call or reverse auth relay.

Successful logins may carry a host-supplied `LoginContext` from the user and
request already authenticated. The shared, targetless payload adds `username`,
`name`, UTC RFC3339 `occurred_at`, and nullable `ip` and browser/device UUID
`identity`. These are login-time facts, not current authorization state. No
extra lookup is needed to build them, and no credentials, session tokens or raw
cookies are exposed. JavaScript subscribers receive the same JSON through the
ordinary ApplicationEvent bridge; they need no auth-specific transport. The
additive fields keep the v1 topic and older context-free payloads valid.
Logout may also carry `username` from the host's existing user lookup; the
field is omitted when that user is unavailable. The PHP transport decoder
exposes the optional username for lightweight observers without a model lookup.

PHP's `Providers\AuthServiceProvider` enables the translator only with explicit
`auth.claviron_events = {provider, subject_key}` configuration. It does not
subscribe every HTTP/queue instance: the consuming application's registrar
declares `Lumenize\\Auth\\Events\\AuthEvent`, and `invoke:serve` starts
that catalog. `DispatchAuthEvent` resolves the subject using the
application's named Laravel user provider and dispatches the actual native
`Illuminate\\Auth\\Events\\Login`, `Logout`, or `Registered` object. Ordinary
listeners and queued notifications remain Laravel's. Unknown/missing subjects
and invalid versions/types fail explicitly; payloads cannot select PHP classes
or supply user models. EventBridge's inbound context suppresses fleet echo.
The PHP decoder otherwise ignores the extra login snapshot fields; the explicit
native translator's user-provider lookup and Laravel event behavior are unchanged.

## Embedded queue workers

The queue provider changes execution ownership, not queue storage: Redis,
database, SQS and other backends remain Laravel application policy. Under the
Claviron PHP host it replaces Laravel's process-signal worker with a worker
that observes the task cancellation token at normal loop boundaries.

A Redis `block_for` wait is outside the PHP VM and cannot observe that token.
The provider therefore normalizes Redis queue connections to non-blocking
polls before Laravel resolves them. An empty poll falls through to Laravel's
ordinary `queue:work --sleep` interval, whose PHP `sleep()` is
cancellation-aware in the embedded runtime. Ordinary PHP CLI and FPM never
enter this provider path and retain the application's configured Redis blocking
policy.

## Native registration ownership

The ext-php-rs 0.16 adapter appends complete builder results to
`php-embed::extension::FunctionEntries`. The SAPI owns the function entry and its
argument metadata together for the process lifetime; the Laravel extension does
not free or separately retain them. Its hand-written broadcast handler uses
`zend::run_handler` for the same panic/bailout boundary as the core host functions.

The `native_registration` test (requiring `broadcast`) exercises the actual
function's reflection metadata and unconfigured-transport exception across ZTS
requests. Its harness remains package-local so the published crate can run it; the test
starts no broadcast fleet, Laravel application or database.
