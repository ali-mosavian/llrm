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

# x86_16 refused every program when LLRM_CLANG_M16 was unset (215 refused, no measurement) though the pinned fork clang sits in ~/scratch/clang-by-sha.
printf '#!/bin/sh\n[ -n "${LLRM_CLANG_M16:-}" ] || exit 1\nwhile [ $# -gt 1 ]; do [ "$1" = -o ] && echo x > "$2"; shift; done\n' > "$t/needs_fork"
chmod +x "$t/needs_fork"
fork=$t/home/scratch/clang-by-sha/$(sed -n 2p "$root/toolchain/clang/pin" | cut -d' ' -f1)/bin; mkdir -p "$fork"; printf '#!/bin/sh\n' > "$fork/clang"; chmod +x "$fork/clang"
(unset LLRM_CLANG_M16; HOME=$t/home; export HOME; run "$t/needs_fork" "$t/needs_fork" x86_16-dos O2 "$t/p.c") || { echo "FAIL: the pinned fork clang was not found"; exit 1; }
# A compile that never ended ran the identity for hours (z80 -O3, 2.5 h) and read as neither a pass nor a refusal: it is counted as hung.
printf '#!/bin/sh\nexec sleep 300\n' > "$t/hang"; chmod +x "$t/hang"
out=$(IDENTITY_TIMEOUT=1 timeout 60 "$root/tools/identity_targets.sh" "$t/ok" "$t/hang" m68k-68000 O2 "$t/p.c" 2>&1) && { echo "FAIL: a hung compile passed"; exit 1; }
case $out in *"HUNG "*"hung=1"*) ;; *) echo "FAIL: a hung compile is not named: $out"; exit 1;; esac
echo ok
