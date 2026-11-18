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

# 36 of the 40 x86_64 "refused" programs were `// targets: dos|x86-m16`; os_*.c and bench_io.c failed on a missing llrm_os.h: a program its header excludes is n/a, the rest get the include dirs.
printf '// targets: dos\nint main(void){return 0;}\n' > "$t/dos_only.c"
printf '#!/bin/sh\necho "$@" >> "$ARGS_LOG"\nwhile [ $# -gt 1 ]; do [ "$1" = -o ] && echo x > "$2"; shift; done\n' > "$t/logs"; chmod +x "$t/logs"
summary=$(ARGS_LOG=$t/args AB_ALLOW_ENV=ARGS_LOG "$root/tools/identity_targets.sh" "$t/logs" "$t/logs" x86_64-linux O2 "$t/p.c" "$t/dos_only.c" 2>&1 | tail -n 1)
case $summary in *"same=1 diff=0 refused=0 status=0 na=1") ;; *) echo "FAIL: a DOS-only program was not n/a: $summary"; exit 1;; esac
grep -q "dos_only" "$t/args" && { echo "FAIL: a DOS-only program was compiled"; exit 1; }
grep -q -- "-I $root/runtime/shared/posix" "$t/args" || { echo "FAIL: the runtime include dirs were not given"; exit 1; }
echo ok
