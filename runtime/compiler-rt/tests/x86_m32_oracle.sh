#!/bin/bash
# usage: [LLRM_CC_DIR=dist] tests/x86_m32_oracle.sh [N] -> sf64 built by llrm-cc -m32 (config/x86_m32/config.h) at -Os and -O2, linked to the gcc -m32 host oracle; must print fails=0.
# The oracle runs on SSE2 (-mfpmath=sse): the default x87 reference double-rounds and reported 20637 false failures (mul/div of denormal-edge operands).
set -e
cd "$(dirname "$0")/.."
D=${LLRM_CC_DIR:?set LLRM_CC_DIR to a dist with llrm-cc and llrm-llc}; export LLRM_LLC=${LLRM_LLC:-$D/llrm-llc}; T=$(mktemp -d); trap 'rm -r "${T:?}"' EXIT
for O in Os O2; do
  for f in $(tests/fp_sources.sh f64); do LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -m32 -$O -include config/x86_m32/config.h -Iinclude '-DSF_NAME(n)=sfb_##n' -c $f -o $T/f64_$(basename $f .c).o; done
  gcc -m32 -msse2 -mfpmath=sse -O2 -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f64.c $T/f64_*.o -o $T/h
  $T/h ${1:-} > $T/out || { echo "m32 f64 -$O: $(tail -1 $T/out)"; exit 1; }; echo "m32 f64 -$O: $(tail -1 $T/out)"
done
