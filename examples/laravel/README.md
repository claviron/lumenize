# Laravel on Lumenize

A small notes application: add a note, mark it done, or delete it. Routes,
validation, forms and database access are ordinary Laravel. There is no
JavaScript build step.

## Prepare

You need Composer, Rust, and the ZTS embed PHP build used by Lumenize, including
PDO SQLite. Use that build's executable as `PHP`, its `php-config` as
`PHP_CONFIG`, and its installation directory as `PHP_DIR`. Do not substitute
an unrelated system PHP: the host and its PHP extensions must match.

From this directory:

```sh
cp .env.example .env
"$PHP" /path/to/composer.phar install
"$PHP" artisan migrate --seed
```

If Composer is installed as a PHP executable, pass its path instead of
`/path/to/composer.phar`. The sample app key is public and for local use only;
`"$PHP" artisan key:generate` replaces it in your ignored `.env` if needed.

The first migration creates `database/database.sqlite` through Laravel. This
is this example's own local database, not a shared Liberum database. Laravel's
PDO connection opens the file inside the PHP process hosted by Lumenize. There
is no Rust SQL pool, separate database service, or database connection over Orbit.
Migrations run only when you ask, never during HTTP startup. Seeding is repeatable.

## Run

From this same directory:

```sh
cargo run --package lumenize-laravel-example
```

Open <http://127.0.0.1:8000>. The first run compiles the small native launcher;
you do not need to change its Rust code. PHP runs in that process, with Laravel's
Octane request handling. Do not also start `php artisan serve` or `octane:start`.
Ctrl-C stops the local runner. The listener is loopback-only and Orbit is off.

Edit `routes/web.php`, the controller, model or Blade view as you normally would.
Restart the runner after changing PHP code loaded by the persistent application.
Configuration lives in `lumenize.yaml`; paths there are relative to this directory.

This is an unauthenticated local example, not a production deployment template.

## Work on Lumenize together with the example

The Rust launcher is an unpublished member of Lumenize's Cargo workspace. Its
`lumenize` dependency inherits the root workspace's local path. Composer's
`claviron/laravel` dependency is a symlink to `../../php`, the integration in
this same repository. Changes can be developed together without publishing.
`claviron/sapi` remains an ordinary released Composer dependency.

`composer.lock` pins the other PHP dependencies. If the local integration's
requirements change, update it explicitly with Composer. Dependencies, compiled
output, the SQLite file and runtime storage are not committed.

For an isolated PHP check after installing dependencies:

```sh
"$PHP" tests/smoke.php
```

It uses a temporary SQLite database and checks the Laravel routes, validation,
CSRF and note changes. It does not compile or start the native runner. Running
the embedded host is a separate check, not something this PHP test proves.

## Add an Edge later

The dev runner works by itself. To put a separate Claviron Edge in front without
changing Laravel code, follow [Connect an Edge with Orbit](edge/README.md).
