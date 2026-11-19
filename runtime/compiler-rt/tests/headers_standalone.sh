#!/bin/sh
# Every lib/*/*.c compiles alone with the host cc: sf_helpers.h once used sf32 before softfloat.h declared it, and int32.c/int64.c (which only include sf_helpers.h) stopped building at m68k.
cd "$(dirname "$0")/.."
bad=0
for f in lib/*/*.c lib/fp/*/*.c; do
  cc -fsyntax-only -Iinclude -DSF_INTABI=1 "$f" 2>/dev/null || { echo "FAIL $f"; bad=1; }
done
# sf_helpers.h once ended its include guard before sf_round3 and pack_near: a second include redefined them
printf '#include "sf_helpers.h"\n#include "sf_helpers.h"\n' > ${TMPDIR:-/tmp}/twice.c
cc -fsyntax-only -Iinclude -DSF_INTABI=1 ${TMPDIR:-/tmp}/twice.c 2>/dev/null || { echo "FAIL sf_helpers.h twice"; bad=1; }
[ $bad = 0 ] && echo "ok: every lib/*/*.c compiles alone, sf_helpers.h twice"
exit $bad
