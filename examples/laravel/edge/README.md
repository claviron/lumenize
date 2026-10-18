# Connect an Edge with Orbit

Start with the direct [development runner](../README.md). You only need this
step when you want a separate Claviron Edge to receive browser requests.

The browser connects to Edge. Edge passes the request to the named Laravel
application through Orbit Link, a shared-memory connection on the same OS.
PHP still runs in the Laravel application's process and opens its own SQLite
file there. Orbit does not carry a database connection.

## 1. Enable Orbit on the application

Stop the example's current dev runner first. From `examples/laravel`, start it
with one additional option:

```sh
cargo run --package lumenize-laravel-example -- --orbit
```

The existing `lumenize.yaml` supplies application name `laravel-example` and
fleet name `lumenize`. Edge uses these two names to find it. The direct loopback
listener remains available; enabling Orbit does not create another PHP application
or move the SQLite database. Do not run a second copy just to add this option.

## 2. Add a host to your Edge

For the supplied local configuration, start Claviron's example `edge-server`
binary in a second terminal, from `examples/laravel`:

```sh
bash edge/run.sh /path/to/edge-server
```

Use a binary built from a compatible Claviron version and matching Orbit
transport settings. This script does not build or download it. It starts only
its own standalone Edge on <http://localhost:8080>, using `edge/config/`.
`localhost` is a local alias of `laravel.test`, so no hosts-file
edit is needed for that URL. Keep the application terminal running too.

The configuration passes all application requests through Orbit, with no TCP
upstream or embedded PHP in Edge. Its request limits reuse Claviron's existing
example Edge policy. SQLite stays in the application process. Stop this Edge
with Ctrl-C when done; the application keeps running.

To use an Edge you already operate instead, do not start another listener on
the same port. Add the host below to that Edge's own vhost document.

In the Edge's vhost document, add the following, replacing the public directory
with the path on your machine:

```yaml
laravel.test:
  root: /path/to/lumenize/examples/laravel/public
  transport:
    non_ssl: serve
    protocols: [h1]
  orbit:
    - { path: /, application: laravel-example, fleet: lumenize }
```

The Edge must be built with Orbit routing support. Keep its existing listener
configuration; this snippet does not configure a port, certificate or another
server. Apply the change using your Edge's normal configuration workflow.

## 3. Open the example through Edge

Resolve `laravel.test` to loopback and open it on your Edge's listener port.
For example, if that listener uses port 8080:

```sh
curl -H 'Host: laravel.test' http://127.0.0.1:8080/
```

The same notes should appear through the direct URL and through Edge. If the
application cannot be reached, check that both processes use the same OS user
and that the application and fleet names match. Orbit Link is not a cross-host
network: a VM and its host are different environments for this purpose.

Keep the listener local for this example. Production TLS, authentication and
deployment are separate decisions. No Edge is started or modified automatically.
