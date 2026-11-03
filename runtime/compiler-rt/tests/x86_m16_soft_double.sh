#!/bin/bash
# usage: [LLRM_CC=path/to/llrm-cc] [TERM_ONLY=add|sub|mul|div|cmp|d2i|i2d|f2d|d2f] tests/x86_m16_soft_double.sh [NV] [BENCH_DIR] -> bench program sfm16_double[_TERM]:
# plain double C over NV vectors with the soft-float library (sf32, sf32_conv, sf64, sf32_conv64; their statics renamed apart) flattened beside it.
# Build with LLRM_BENCH_FLAGS=-mno-80387; the expected output is the same program on the host FPU (SSE). With LLRM_CC the compiled function must
# use no x87 op and call the routines.
set -e
cd "$(dirname "$0")/.."
NV=${1:-400}; B=${2:-$(cd ../.. && pwd)/bench}; T=$(mktemp -d); trap 'rm -r $T' EXIT
TERM=${TERM_ONLY:-all}; n=sfm16_double${TERM_ONLY:+_$TERM_ONLY}; mkdir -p $B/$n
cc -O2 -ffp-contract=off tests/gen_vectors64.c -o $T/gen && $T/gen $NV | sed 's/uint32_t/unsigned long/' > $T/vectors.h
printf '#include <stdio.h>\nvoid report(long v) { printf("%%ld\\n", v); }\n' > $T/rep.c
python3 - "$T" <<'P'
import re, subprocess, sys
sys.path.insert(0, "tests")
from flatten import flatten
T = sys.argv[1]
def prep(path, suffix=''):
    s = flatten(path, suffix)
    s = re.sub(r'#include <(stdint|stddef|limits)\.h>\n', '', s)
    s = re.sub(r'#include "(softfloat|sf_helpers|sf_int)\.h"\n', '', s)
    for a, b in (('uint8_t', 'sfu8'), ('int8_t', 'sfi8'), ('uint16_t', 'sfu16'), ('int16_t', 'sfi16'), ('uint32_t', 'sfu32'), ('int32_t', 'sfi32'), ('uint64_t', 'sfu64'), ('int64_t', 'sfi64')):
        s = re.sub(r'\b%s\b' % a, b, s)
    return s
out = ['#define SF_INTABI 1']
out.append(prep('config/x86_m16/config.h'))
for f in ('include/softfloat.h', 'include/sf_helpers.h'):
    out.append(prep(f))
for f in subprocess.check_output(['tests/fp_sources.sh', 'f32'], text=True).split():
    out.append(prep(f))
out.append(prep('lib/int/clzsi2.c'))
for f in subprocess.check_output(['tests/fp_sources.sh', 'f64'], text=True).split():
    out.append(prep(f, '_d'))
for f in subprocess.check_output(['tests/fp_sources.sh', 'f32_64'], text=True).split():
    out.append(prep(f, '_e'))
open(T + '/lib.c', 'w').write('\n'.join(out))
P
{ echo 'extern void report(long value);'
  echo 'typedef unsigned char sfu8; typedef signed char sfi8; typedef short sfi16; typedef unsigned short sfu16; typedef long sfi32; typedef unsigned long sfu32; typedef unsigned long long sfu64; typedef long long sfi64;'
  echo '#define UINT32_C(x) x##ul'; echo '#define UINT64_C(x) x##ull'; echo '#define INT32_MAX 2147483647L'; echo '#define INT32_MIN (-2147483647L-1)'; echo '#define UINT32_MAX 4294967295ul'
  echo '#define UINT64_MAX 18446744073709551615ull'; echo '#define INT64_MAX 9223372036854775807ll'; echo '#define INT64_MIN (-9223372036854775807ll-1)'
  cat $T/lib.c
  cat $T/vectors.h
  cat <<'C'
static void *const keep[] = { (void *)__adddf3, (void *)__subdf3, (void *)__muldf3, (void *)__divdf3, (void *)__eqdf2, (void *)__nedf2,
                              (void *)__ltdf2, (void *)__ledf2, (void *)__gtdf2, (void *)__gedf2, (void *)__floatsidf, (void *)__fixdfsi,
                              (void *)__extendsfdf2, (void *)__truncdfsf2, (void *)__floatsisf, (void *)__fixsfsi };
static double bits(unsigned long hi, unsigned long lo) { union { unsigned long long u; double d; } x; x.u = ((unsigned long long)hi << 32) | lo; return x.d; }
static unsigned long canon(double d)
{
    union { unsigned long long u; double d; } x;
    if (d != d) return 0x7FF80000ul;
    x.d = d;
    return (unsigned long)(x.u >> 32) + (unsigned long)x.u * 3ul;
}
static unsigned long canonf(float f) { union { unsigned long u; float f; } x; if (f != f) return 0x7FC00000ul; x.f = f; return x.u; }
long bench_NAME(int n)
{
    int i;
    unsigned long acc = 0;
    for (i = 0; i < n; ++i) {
        double a = bits(V[i][0], V[i][1]), b = bits(V[i][2], V[i][3]);
        acc += W_ADD * canon(a + b) + 3 * W_SUB * canon(a - b) + 5 * W_MUL * canon(a * b) + 7 * W_DIV * canon(a / b);
        if (W_CMP) acc += (a < b) + 2 * (a == b) + 4 * (a > b) + 8 * (a != b) + 16 * (a <= b) + 32 * (a >= b);
        if (W_D2I && ((V[i][0] >> 20) & 0x7FF) < 1023 + 30) acc += (long)a;
        if (W_I2D) acc += canon((double)(long)V[i][3]);
        if (W_F2D) acc += canon((double)bits_f(V[i][1]));
        if (W_D2F) acc += canonf((float)a);
    }
    return (long)acc;
}
int main(void)
{
    static void *volatile sink;
    int i;
    for (i = 0; i < 16; ++i) sink = keep[i];
    report(bench_NAME(NV));
    return 0;
}
C
} > $B/$n/$n.c
sed -i 's/bits_f(/bitsf(/' $B/$n/$n.c
sed -i 's|^static unsigned long canonf|static float bitsf(unsigned long u) { union { unsigned long u; float f; } x; x.u = u; return x.f; }\nstatic unsigned long canonf|' $B/$n/$n.c
w() { [ "$TERM" = all ] || [ "$TERM" = "$1" ] && echo 1 || echo 0; }
sed -i "s/bench_NAME/bench_$n/g" $B/$n/$n.c
sed -i "1i #define W_ADD $(w add)\n#define W_SUB $(w sub)\n#define W_MUL $(w mul)\n#define W_DIV $(w div)\n#define W_CMP $(w cmp)\n#define W_D2I $(w d2i)\n#define W_I2D $(w i2d)\n#define W_F2D $(w f2d)\n#define W_D2F $(w d2f)" $B/$n/$n.c
printf 'description = "%s: plain double C over %s vectors, soft-float library flattened"\nrepetitions = 1\n\n[region]\nc = "bench_%s"\n' "$n" "$NV" "$n" > $B/$n/bench.toml
gcc -m32 -msse2 -mfpmath=sse -O0 -w $B/$n/$n.c $T/rep.c -o $T/ref && $T/ref > $B/$n/$n.out
echo "wrote $B/$n expected $(cat $B/$n/$n.out)"
if [ -n "$LLRM_CC" ]; then
  $LLRM_CC --target=x86_16-wat -march=i386 -O2 -mno-80387 -S -o $T/f.s $B/$n/$n.c
  x87=$(awk -v fn="bench_${n}" '$0 ~ "^_" fn "@3 proc" {p=1} p && /endp/ {exit} p {print}' $T/f.s | grep -c -E '^\s+f(ld|st|add|sub|mul|div|comp|ild|istp|xch)' || true)
  calls=$(grep -c -E 'call.*__(add|sub|mul|div)df3' $T/f.s || true)
  echo "x87 ops $x87, soft-float calls $calls"
  [ "$x87" -eq 0 ]
fi
