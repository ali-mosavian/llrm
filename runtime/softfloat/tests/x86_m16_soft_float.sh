#!/bin/bash
# usage: [LLRM_CC=path/to/llrm-cc] tests/x86_m16_soft_float.sh [NV] [BENCH_DIR] -> bench program sfm16_float: plain float C (+ - * / compares, int<->float) over NV vectors with the
# soft-float library flattened beside it (SF_INTABI). Build with LLRM_BENCH_FLAGS=-mno-80387; the expected output is the same program on the host FPU (SSE, round to nearest).
set -e
cd "$(dirname "$0")/.."
NV=${1:-400}; SF_M16=$(sed -E "s/\buint(32|64)_t\b/sfu\1/g" include/sf_x86_m16.h); export SF_M16; B=${2:-$(cd ../.. && pwd)/bench}; T=$(mktemp -d); trap 'rm -r $T' EXIT
TERM=${TERM_ONLY:-all}; n=sfm16_float${TERM_ONLY:+_$TERM_ONLY}; mkdir -p $B/$n
cc -O2 -ffp-contract=off tests/gen_vectors.c -o $T/gen && $T/gen $NV | sed 's/uint32_t/unsigned long/' > $T/vectors.h
printf '#include <stdio.h>\nvoid report(long v) { printf("%%ld\\n", v); }\n' > $T/rep.c
{ echo 'extern void report(long value);'
  echo 'typedef unsigned char sfu8; typedef signed char sfi8; typedef short sfi16; typedef unsigned short sfu16; typedef long sfi32; typedef unsigned long sfu32; typedef unsigned long long sfu64; typedef long long sfi64;'
  echo '#define UINT32_C(x) x##ul'; echo '#define UINT64_C(x) x##ull'; echo '#define INT32_MAX 2147483647L'; echo '#define INT32_MIN (-2147483647L-1)'; echo '#define UINT32_MAX 4294967295ul'
  echo '#ifdef __SOFT_LIB'
  for f in include/softfloat.h include/sf_helpers.h src/sf32.c src/sf32_conv.c; do
    sed -E '/#include <(stdint|stddef|limits)\.h>/d; /#include "(softfloat|sf_helpers|sf_int)\.h"/d; s/\buint8_t\b/sfu8/g; s/\bint8_t\b/sfi8/g; s/\buint16_t\b/sfu16/g; s/\bint16_t\b/sfi16/g; s/\buint32_t\b/sfu32/g; s/\bint32_t\b/sfi32/g; s/\buint64_t\b/sfu64/g; s/\bint64_t\b/sfi64/g' $f
  done | awk 'BEGIN{print "#define SF_INTABI 1"; print ENVIRON["SF_M16"]} {print}'
  echo '#endif'
  cat $T/vectors.h
  cat <<'C'
/* The compiler calls these for the float operations after the program is whole: referenced here, the dead-code pass keeps them. */
static void *const keep[] = { (void *)__addsf3, (void *)__subsf3, (void *)__mulsf3, (void *)__divsf3, (void *)__eqsf2, (void *)__nesf2,
                              (void *)__ltsf2, (void *)__lesf2, (void *)__gtsf2, (void *)__gesf2, (void *)__floatsisf, (void *)__fixsfsi };
static float bits(unsigned long u) { union { unsigned long u; float f; } x; x.u = u; return x.f; }
static unsigned long canon(float f) { union { unsigned long u; float f; } x; if (f != f) return 0x7FC00000ul; x.f = f; return x.u; }
long bench_NAME(int n)
{
    int i;
    unsigned long acc = 0;
    for (i = 0; i < n; ++i) {
        float a = bits(V[i][0]), b = bits(V[i][1]);
        acc += W_ADD * canon(a + b) + 3 * W_SUB * canon(a - b) + 5 * W_MUL * canon(a * b) + 7 * W_DIV * canon(a / b);
        if (W_CMP) acc += (a < b) + 2 * (a == b) + 4 * (a > b) + 8 * (a != b) + 16 * (a <= b) + 32 * (a >= b);
        if (W_F2I && ((V[i][0] >> 23) & 0xFF) < 127 + 30) acc += (long)a;
        if (W_I2F) acc += canon((float)(long)V[i][1]);
    }
    return (long)acc;
}
int main(void)
{
    static void *volatile sink;
    int i;
    for (i = 0; i < 12; ++i) sink = keep[i];
    report(bench_NAME(NV));
    return 0;
}
C
} > $B/$n/$n.c
w() { [ "$TERM" = all ] || [ "$TERM" = "$1" ] && echo 1 || echo 0; }
sed -i "s/bench_NAME/bench_$n/g" $B/$n/$n.c
sed -i "1i #define W_ADD $(w add)\n#define W_SUB $(w sub)\n#define W_MUL $(w mul)\n#define W_DIV $(w div)\n#define W_CMP $(w cmp)\n#define W_F2I $(w f2i)\n#define W_I2F $(w i2f)" $B/$n/$n.c
printf 'description = "%s: plain float C over %s vectors, soft-float library flattened"\nrepetitions = 1\n\n[region]\nc = "bench_%s"\n' "$n" "$NV" "$n" > $B/$n/bench.toml
sed -i '1i #define __SOFT_LIB 1' $B/$n/$n.c
gcc -m32 -msse2 -mfpmath=sse -O0 -w -U__SOFT_LIB $B/$n/$n.c $T/rep.c -o $T/ref && $T/ref > $B/$n/$n.out
echo "wrote $B/$n expected $(cat $B/$n/$n.out)"
# A program that matches the host proves nothing if it ran on the x87: with LLRM_CC, the compiled function must call the routines and use no x87 op.
if [ -n "$LLRM_CC" ]; then
  $LLRM_CC --target=x86_16-wat -march=i386 -O2 -mno-80387 -S -o $T/f.s $B/$n/$n.c
  x87=$(awk '/^bench_${n}:/,/^[a-z_0-9]+:$/ && !/^bench_${n}:/ {exit} {print}' $T/f.s | grep -c -E '^\s+f(ld|st|add|sub|mul|div|comp|ild|istp|xch)' || true)
  calls=$(grep -c -E 'call.*__(add|sub|mul|div)sf3' $T/f.s || true)
  echo "x87 ops $x87, soft-float calls $calls"
  [ "$x87" -eq 0 ] && [ "$calls" -gt 0 ]
fi
