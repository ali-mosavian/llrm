#!/bin/bash
# usage: [GEN=tests/x86_m16_soft_float.sh] tests/x86_m16_soft_float_test.sh
# The generator flattened the library with config/x86_m16's SF_CMP_SPLIT still set: every per-entry cmp file redefined sf_cmp_un, the programs did not compile, and the
# committed sfm16_float* kept the 2026-11-03 sf32.c snapshot (cells measured on the wrong library). Each generated program must compile as C (library on) and carry
# `references = false` like the committed bench.toml; the host output of the add program must equal the committed .out.
cd "$(dirname "$0")/.." || exit 2
G=${GEN:-tests/x86_m16_soft_float.sh}; T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT; bad=0; B=$(cd ../.. && pwd)/bench
for t in add cmp; do TERM_ONLY=$t bash $G 400 $T > $T/gen.log 2>&1 || { echo "FAIL: generator $t exited non-zero"; bad=1; continue; }
  n=sfm16_float_$t
  gcc -m32 -fsyntax-only -w $T/$n/$n.c 2> $T/err || { echo "FAIL: $n does not compile: $(head -1 $T/err)"; bad=1; }
  grep -q '^references = false' $T/$n/bench.toml || { echo "FAIL: $n/bench.toml lacks references = false"; bad=1; }
  cmp -s $T/$n/$n.out $B/$n/$n.out || { echo "FAIL: $n.out differs from the committed one"; bad=1; }
done
[ $bad = 0 ] && echo "x86_m16_soft_float ok"; exit $bad
