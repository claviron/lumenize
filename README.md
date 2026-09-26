# Lumenize

*For the small, fast spirit Lumen left behind.*

Lumenize brings Safe Atomic Rust Power to PHP/Laravel's breeze of magic.

A Laravel-focused PHP runner powered by Claviron.

**This release is a name-reservation placeholder. It contains no working
runner, starter kit, or public API.**

## Direction

Start with a familiar Laravel starter-kit experience. Grow into an application
that owns its runtime: your Laravel code, your Rust `main.rs`, your binary.

Laravel expresses application intent through Eloquent, validation, controllers,
and business orchestration. Rust and Claviron provide the runtime around it:
embedded PHP, supervised workers and tasks, database pools, and Orbit-backed
shared state and events.

The planned runner composes Claviron capabilities to fit the application. It can
start small and grow to include edge services, upstream routing, or embedded
JavaScript without requiring every application to carry every capability.

PHP keeps its request-lifetime ergonomics. The application owns the durable
runtime around it. These are design goals, not features of this placeholder.

## Non-Affiliation

This is an independent project, not affiliated with or endorsed by Laravel,
Lumen, or the official Illuminate components. It is not a Laravel replacement.

## License

MIT.
