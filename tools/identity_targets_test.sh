#!/usr/bin/env bash
# Regression tests of identity_targets.sh.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
t=$(mktemp -d); trap 'rm -rf "$t"' EXIT
printf '#!/bin/sh\nwhile [ $# -gt 1 ]; do [ "$1" = -o ] && echo x > "$2"; shift; done\n' > "$t/ok"
printf '#!/bin/sh\nexit 1\n' > "$t/refuse"
printf '#!/bin/sh\nwhile [ $# -gt 1 ]; do [ "$1" = -o ] && echo y > "$2"; shift; done\n' > "$t/other"
chmod +x "$t"/ok "$t"/refuse "$t"/other; echo 'int main(void){return 0;}' > "$t/p.c"
run() { "$root/tools/identity_targets.sh" "$@" > /dev/null 2>&1; }

# An a64 run whose cell list came out empty printed "done" and read as a pass: no program compared must fail.
run "$t/ok" "$t/ok" m68k-68000 O2 "$t/none.c" && { echo "FAIL: empty set passed"; exit 1; }
run "$t/refuse" "$t/refuse" m68k-68000 O2 "$t/p.c" && { echo "FAIL: all-refused set passed"; exit 1; }
run "$t/ok" "$t/ok" m68k-68000 O2 "$t/p.c" || { echo "FAIL: identical build failed"; exit 1; }
run "$t/ok" "$t/other" m68k-68000 O2 "$t/p.c" && { echo "FAIL: DIFF passed"; exit 1; }
run "$t/ok" "$t/refuse" m68k-68000 O2 "$t/p.c" && { echo "FAIL: STATUS passed"; exit 1; }
echo ok
