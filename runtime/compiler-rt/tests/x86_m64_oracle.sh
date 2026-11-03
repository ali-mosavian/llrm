#!/bin/bash
# usage: tests/x86_m64_oracle.sh [N] -> sf32/sf64 built by llrm-cc -m64 with config/x86_m64/config.h, linked to the gcc-built host oracle; must print no mismatches.
# Needs LLRM_CC_DIR (llrm-cc). Guards the __int128 mul64: a wrong high half fails the mul sweeps.
set -e
cd "$(dirname "$0")/.."
D=${LLRM_CC_DIR:-$HOME/targets/gs-out/rbase}; T=$(mktemp -d); trap 'rm -r $T' EXIT
for O in Os O2; do
  for f in $(tests/fp_sources.sh f64); do LLRM_CC_TARGET=x86_64-linux $D/llrm-cc -m64 -$O -include config/x86_m64/config.h -Iinclude '-DSF_NAME(n)=sfb_##n' -c $f -o $T/f64_$(basename $f .c).o; done
  gcc -O2 -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f64.c $T/f64_*.o -o $T/h
  echo "m64 f64 -$O: $($T/h ${1:-} | tail -1)"
  for f in $(tests/fp_sources.sh f32 f32_64); do LLRM_CC_TARGET=x86_64-linux $D/llrm-cc -m64 -$O -include config/x86_m64/config.h -Iinclude '-DSF_NAME(n)=sfb_##n' -c $f -o $T/f32_$(basename $f .c).o; done
  gcc -O2 -fno-builtin -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f32.c $T/f32_*.o -o $T/h32
  echo "m64 f32 -$O: $($T/h32 ${1:-} | tail -1)"
done
