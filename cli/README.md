# Lumenize console

`lumenize-cli` is the executable package for the `lumenize` Artisan launcher.
It uses the root library's independent `cli` feature, without selecting its
HTTP `runner`, supervisor, listeners or Orbit composition.

```sh
cargo install lumenize-cli --version 0.2.0-alpha.20 --locked
# From a Laravel project root:
lumenize list
lumenize migrate --pretend
```

For a local checkout, use `cargo install --path cli --bin lumenize`.
The package's default `cli` feature forwards to `lumenize/cli`; the binary only
maps the command's result to an exit status. UTF-8 arguments, environment,
terminal streams and cooperative Ctrl-C cancellation are owned by the library.
Cancellation joins the PHP execution instead of detaching it.

Applications embedding the serving API depend on `lumenize` with `runner`,
not on this package. See the [Runner guide](../docs/runner/README.md) and
[integration guide](../docs/README.md).

Alpha APIs may change without backward compatibility guarantees. MIT license.
