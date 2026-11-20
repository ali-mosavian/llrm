#!/usr/bin/env bash
# tip-static.sh [DIR]: the static scans of a checkout (default: here), in seconds, no build: a stale generated page, a tracked file naming a machine path, a developer's home directory
# or the assistant. Prints the STOP line and exits 1 on the first. The lane scripts call it on the merged head before they land it.
cd "$(dirname "$0")/../.." || exit 2
source tools/landing/red-tip.sh
tip_scanners "${1:-.}"
