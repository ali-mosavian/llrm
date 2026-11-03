#!/bin/sh
# Every src/*.c compiles alone with the host cc: sf_helpers.h once used sf32 before softfloat.h declared it, and int32.c/int64.c (which only include sf_helpers.h) stopped building at m68k.
cd "$(dirname "$0")/.."
bad=0
for f in src/*.c; do
  cc -fsyntax-only -Iinclude -DSF_INTABI=1 "$f" 2>/dev/null || { echo "FAIL $f"; bad=1; }
done
[ $bad = 0 ] && echo "ok: every src/*.c compiles alone"
exit $bad
