#!/bin/sh
# usage: tests/fp_sources.sh GROUP... -> the C files of each group, relative to runtime/compiler-rt.
# f32 (in this order: the flattened one-file benches are register-allocation order sensitive, and the kernels came first in the former sf32.c): binary32 without a 64-bit type; f32_64: its 64-bit integer and binary64 conversions; f64; f128 (binary128, needs a 128-bit integer type); f8; int; int64 (one routine per file, __muldi3 .. __ashrdi3 and their divide worker); math.
cd "$(dirname "$0")/.."
for g in "$@"; do case $g in
  f32) printf "%s\n" lib/fp/f32/mul.c lib/fp/f32/div.c lib/fp/f32/add_sub.c lib/fp/f32/cmp.c lib/fp/f32/cmp_cmp.c lib/fp/f32/cmp_eq.c lib/fp/f32/cmp_ne.c lib/fp/f32/cmp_lt.c lib/fp/f32/cmp_le.c lib/fp/f32/cmp_ge.c lib/fp/f32/cmp_gt.c lib/fp/f32/unord.c lib/fp/f32/neg.c lib/fp/f32/pack.c lib/fp/f32/float_si.c lib/fp/f32/float_usi.c lib/fp/f32/fix_si.c lib/fp/f32/fix_usi.c;;
  f32_64) printf "%s\n" lib/fp/f32/extend.c lib/fp/f32/fix_di.c lib/fp/f32/float_di.c lib/fp/f32/trunc.c;;
  f64) ls lib/fp/f64/*.c;;
  f128) ls lib/fp/f128/*.c;;
  f8) ls lib/fp/f8/*.c;;
  int) ls lib/int/*.c;;
  int64) ls lib/int/muldi3.c lib/int/udivmod_w.c lib/int/udivdi3.c lib/int/umoddi3.c lib/int/divdi3.c lib/int/moddi3.c lib/int/ashldi3.c lib/int/lshrdi3.c lib/int/ashrdi3.c;;
  clz) ls lib/int/clzsi2.c;; # the shared leading-zero count SF_CLZ_CALL targets call
  math) ls lib/math/*.c;;
  *) echo "fp_sources.sh: unknown group $g" >&2; exit 1;;
esac; done
