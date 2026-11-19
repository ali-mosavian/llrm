#!/bin/sh
# A tracked file named "-" (a tool's stdout capture swept in by `git add -A`) sat on release/next from #805 to 2026-11-09.
bad=$(git ls-files | awk '$0 == "-" || $0 ~ /^-[^\/]*$/')
[ -z "$bad" ] || { echo "FAIL: tracked files named like options: $bad"; exit 1; }
echo ok
