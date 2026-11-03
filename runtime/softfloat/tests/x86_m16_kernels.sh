#!/bin/bash
# usage: tests/x86_m16_kernels.sh [NV] [BENCH_DIR] -> bench programs sfm16_{add,mul,div} (ours) and sfm16crt_{add,mul,div} (compiler-rt) in BENCH_DIR
# (default <repo>/bench) for tools/bench/bench.py --lang c: NV vectors from tests/gen_vectors.c, flattened to one freestanding file with 32-bit long,
# ours uses include/sf_x86_m16.h unless SF_EXTRA is set (SF_EXTRA= alone: the library defaults); SF_EXTRA="#define SF_DIV_NEWTON 1" adds a line to the flattened ours. Expected output from the same file run natively at -O0. compiler-rt is preprocessed for a 16-bit int (msp430) so its types match llrm-cc's x86_16.
set -e
cd "$(dirname "$0")/.."
NV=${1:-400}; SF_EXTRA=${SF_EXTRA-$(sed -E 's/\buint(32|64)_t\b/sfu\1/g' include/sf_x86_m16.h)}; export SF_EXTRA; B=${2:-$(cd ../.. && pwd)/bench}; T=$(mktemp -d); trap 'rm -r $T' EXIT
P=${LLRM_CLANG_M16:-/opt/llrm-pin/bin/clang}; CRT=${COMPILER_RT:-$HOME/scratch/llvm-src-m16/compiler-rt/lib/builtins}
cc -O2 -ffp-contract=off tests/gen_vectors.c -o $T/gen && $T/gen $NV | sed 's/uint32_t/unsigned long/' > $T/vectors.h
printf '#include <stdio.h>\nvoid report(long v) { printf("%%ld\\n", v); }\n' > $T/rep.c
TYPES='typedef unsigned char sfu8; typedef signed char sfi8; typedef short sfi16; typedef unsigned short sfu16; typedef long sfi32; typedef unsigned long sfu32; typedef unsigned long long sfu64; typedef long long sfi64;'
emit() { # name, body-producing function already run into $T/body.c
  n=$1; mkdir -p $B/$n
  { echo 'extern void report(long value);'; cat $T/body.c; cat $T/vectors.h
    echo "long bench_$n(int n) { int i; unsigned long acc = 0; for (i = 0; i < n; ++i) acc += CALL(V[i % NV][0], V[i % NV][1]); return (long)acc; }"
    echo "int main(void) { report(bench_$n($NV)); return 0; }"; } > $B/$n/$n.c
  printf 'description = "%s: %s over %s vectors"\nrepetitions = 1\n\n[region]\nc = "bench_%s"\n' "$n" "$2" "$NV" "$n" > $B/$n/bench.toml
  gcc -m32 -O0 -w -DCALL=$3 $B/$n/$n.c $T/rep.c -o $T/ref && $T/ref > $B/$n/$n.out
  sed -i "1i #define CALL $3" $B/$n/$n.c
}
for op in add mul div; do
  { echo "$TYPES"; echo '#define UINT32_C(x) x##ul'; echo '#define UINT64_C(x) x##ull'; echo '#define INT32_MAX 2147483647L'; echo '#define INT32_MIN (-2147483647L-1)'; echo '#define UINT32_MAX 4294967295ul'
    for f in include/softfloat.h include/sf_helpers.h src/sf32.c src/sf32_conv.c; do
      awk -f tests/inline_inc.awk $f | sed -E '/#include <(stdint|stddef|limits)\.h>/d; /#include "(softfloat|sf_helpers|sf_int)\.h"/d; s/\buint8_t\b/sfu8/g; s/\bint8_t\b/sfi8/g; s/\buint16_t\b/sfu16/g; s/\bint16_t\b/sfi16/g; s/\buint32_t\b/sfu32/g; s/\bint32_t\b/sfi32/g; s/\buint64_t\b/sfu64/g; s/\bint64_t\b/sfi64/g'
    done | awk 'BEGIN{print "#define SF_INTABI 1"; if (ENVIRON["SF_EXTRA"]) print ENVIRON["SF_EXTRA"]} {print}'
    echo "sfu32 __${op}sf3(sfu32, sfu32);"; } > $T/body.c
  sed -i 's/unsigned long/sfu32/' $T/vectors.h 2>/dev/null || true
  emit sfm16_$op "ours $op" __${op}sf3
  sed -i 's/sfu32/unsigned long/' $T/vectors.h
  { $P --target=msp430 -ffreestanding -E -P -I$CRT $CRT/${op}sf3.c | sed '/Qcomplex/d'
    echo 'int __fe_raise_inexact(void) { return 0; }'
    [ $op = add ] && echo 'CRT_FE_ROUND_MODE __fe_getround(void) { return CRT_FE_TONEAREST; }' || echo 'int __fe_getround(void) { return 0; }'
    echo "static float u2f(unsigned long u) { union { unsigned long u; float f; } x; x.u = u; return x.f; } static unsigned long f2u(float f) { union { unsigned long u; float f; } x; x.f = f; return x.u; }"
    echo "#define CALL2(a, b) f2u(__${op}sf3(u2f(a), u2f(b)))"; } > $T/body.c
  emit sfm16crt_$op "compiler-rt $op" CALL2
done
# one-argument and compare routines (inputs of magnitude >= 2^31 or NaN skipped for the float-to-int ones: C leaves them undefined, the libraries differ, and the host reference of the 16-bit-int compiler-rt source is not the target's): name, compiler-rt source, ours call, compiler-rt call (a, b are the vector words)
for row in 'le|comparesf2|__lesf2(a,b)|__lesf2(u2f(a),u2f(b))' 'ge|comparesf2|__gesf2(a,b)|__gesf2(u2f(a),u2f(b))' 'i2f|floatsisf|__floatsisf((sfi32)(a))|f2u(__floatsisf((long)(a)))' 'u2f|floatunsisf|__floatunsisf(a)|f2u(__floatunsisf(a))' 'f2i|fixsfsi|(NAN_(a)?0:__fixsfsi(a))|(NAN_(a)?0:__fixsfsi(u2f(a)))' 'f2u|fixunssfsi|(NAN_(a)?0:__fixunssfsi(a))|(NAN_(a)?0:__fixunssfsi(u2f(a)))'; do
  IFS='|' read -r op crtsrc ours crtc <<<"$row"
  { echo "$TYPES"; echo '#define UINT32_C(x) x##ul'; echo '#define UINT64_C(x) x##ull'; echo '#define INT32_MAX 2147483647L'; echo '#define INT32_MIN (-2147483647L-1)'; echo '#define UINT32_MAX 4294967295ul'
    for f in include/softfloat.h include/sf_helpers.h src/sf32.c src/sf32_conv.c; do
      awk -f tests/inline_inc.awk $f | sed -E '/#include <(stdint|stddef|limits)\.h>/d; /#include "(softfloat|sf_helpers|sf_int)\.h"/d; s/\buint8_t\b/sfu8/g; s/\bint8_t\b/sfi8/g; s/\buint16_t\b/sfu16/g; s/\bint16_t\b/sfi16/g; s/\buint32_t\b/sfu32/g; s/\bint32_t\b/sfi32/g; s/\buint64_t\b/sfu64/g; s/\bint64_t\b/sfi64/g'
    done | awk 'BEGIN{print "#define SF_INTABI 1"; if (ENVIRON["SF_EXTRA"]) print ENVIRON["SF_EXTRA"]} {print}'
    echo "#define NAN_(a) ((((a) >> 23) & 0xFF) >= 158)"; echo "#define CALL1(a, b) ((unsigned long)($ours))"; } > $T/body.c
  sed -i 's/unsigned long/sfu32/' $T/vectors.h 2>/dev/null || true
  emit sfm16_$op "ours $op" CALL1
  sed -i 's/sfu32/unsigned long/' $T/vectors.h
  { $P --target=msp430 -ffreestanding -E -P -I$CRT $CRT/$crtsrc.c | sed '/Qcomplex/d; /__alias__/d'
    echo 'int __fe_raise_inexact(void) { return 0; }'; echo 'int __fe_getround(void) { return 0; }'
    echo "static float u2f(unsigned long u) { union { unsigned long u; float f; } x; x.u = u; return x.f; } static unsigned long f2u(float f) { union { unsigned long u; float f; } x; x.f = f; return x.u; }"
    echo "#define NAN_(a) ((((a) >> 23) & 0xFF) >= 158)"; echo "#define CALL3(a, b) ((unsigned long)($crtc))"; } > $T/body.c
  emit sfm16crt_$op "compiler-rt $op" CALL3
done
echo "wrote $B/sfm16_{add,mul,div} $B/sfm16crt_{add,mul,div}"
