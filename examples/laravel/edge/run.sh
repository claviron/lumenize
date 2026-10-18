#!/usr/bin/env bash
set -eo pipefail

if [[ $# -ne 1 ]]; then
    echo 'Usage: edge/run.sh /path/to/edge-server' >&2
    exit 2
fi

LARAVEL_EXAMPLE_ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
export LARAVEL_EXAMPLE_ROOT

# Start a separate local Edge; never build, stop or reconfigure another server.
exec "$1" --config "$LARAVEL_EXAMPLE_ROOT/edge/config" \
    --environment local --name laravel-example-edge --fleet lmnedg --workers 0
