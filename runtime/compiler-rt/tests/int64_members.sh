#!/bin/sh
# usage: tests/int64_members.sh [FILE...] -> each 64-bit integer file defines exactly one routine (plus udivmod_w for the divides), so a link pulls only what it calls.
# Before the split lib/int/int64.c defined all eight: a shift-only 6502 link carried 11281 B including the divide.
cd "$(dirname "$0")/.."; T=$(mktemp -d); trap 'rm -r $T' EXIT; bad=0
[ $# -gt 0 ] || set -- $(tests/fp_sources.sh int64)
for f in "$@"; do
  cc -c -O2 -fno-builtin -Iinclude -DSF_INTABI "$f" -o $T/o.o || { echo "FAIL $f does not build"; bad=1; continue; }
  n=$(nm --defined-only -g $T/o.o | wc -l)
  [ "$n" = 1 ] || { echo "FAIL $f defines $n global symbols, want 1: $(nm --defined-only -g $T/o.o | awk '{print $3}' | tr '\n' ' ')"; bad=1; }
done
[ $bad = 0 ] && echo "ok: $# file(s), one global symbol each"
exit $bad
