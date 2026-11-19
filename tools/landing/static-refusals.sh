#!/bin/bash
# static-refusals.sh HEAD [--base B] [--title T] [--body-file F]: the refusals every landing path must run before it merges a head (assistant names, conflict markers, deleted-page links,
# subject rules). Prints REFUSED lines, exit 1 if any. `precheck` runs the same and more, but needs gh and a dev checkout; the lane scripts call this one.
cd "$(dirname "$0")/../.." && exec uv run -q --project tools python tools/parity_squash.py static-refusals "$@"
