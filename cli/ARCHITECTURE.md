# Console executable boundary

This package owns only `src/bin/lumenize.rs`, a Tokio entry that delegates to
`lumenize::cli::run()` and maps its exit status. It exports no Rust library.

The default `cli` feature forwards to `lumenize/cli`, independently of the
root library's `runner` feature. Artisan argument handling, PHP execution and
cooperative cancellation remain in the root library. No server boot, listener,
supervisor or fleet attachment is added here.

The optional application-serving API is documented in
[Runner architecture](../docs/runner/ARCHITECTURE.md).
