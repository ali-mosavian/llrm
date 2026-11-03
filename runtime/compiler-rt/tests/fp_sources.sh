#!/bin/sh
# usage: tests/fp_sources.sh GROUP... -> the C files of each group, relative to runtime/compiler-rt.
# f32 (in this order: the flattened one-file benches are register-allocation order sensitive, and the kernels came first in the former sf32.c): binary32 without a 64-bit type; f32_64: its 64-bit integer and binary64 conversions; f64; f8; int; math.
cd "$(dirname "$0")/.."
for g in "$@"; do case $g in
  f32) printf "%s\n" lib/fp/f32/mul.c lib/fp/f32/div.c lib/fp/f32/add_sub.c lib/fp/f32/cmp.c lib/fp/f32/neg.c lib/fp/f32/pack.c lib/fp/f32/float_si.c lib/fp/f32/float_usi.c lib/fp/f32/fix_si.c lib/fp/f32/fix_usi.c;;
  f32_64) printf "%s\n" lib/fp/f32/extend.c lib/fp/f32/fix_di.c lib/fp/f32/float_di.c lib/fp/f32/trunc.c;;
  f64) ls lib/fp/f64/*.c;;
  f8) ls lib/fp/f8/*.c;;
  int) ls lib/int/*.c;;
  math) ls lib/math/*.c;;
  *) echo "fp_sources.sh: unknown group $g" >&2; exit 1;;
esac; done
