#!/bin/sh
# SF_DIV_NEWTON must stay exact against the host FPU on the 60000 vectors (a divisor with a power-of-two mantissa made the reciprocal wrap
# to a quotient off by 2^28 until the clamp), with the default multiply and with a widening one.
cd "$(dirname "$0")/.." || exit 1
T=$(mktemp -d); trap 'rm -r $T' EXIT
run() {
  cc -O2 -fno-builtin -ffp-contract=off -DSF_DIV_NEWTON "$@" -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f32.c src/sf32.c src/sf32_conv.c src/sf32_conv64.c -lm -o $T/h || exit 1
  $T/h | tail -1 | grep -qx 'fails=0' || { echo "SF_DIV_NEWTON $*: mismatches"; exit 1; }
}
run
run '-DSF_MUL32X32(a,b,h,l)=do{unsigned long long p_=(unsigned long long)(a)*(b);*(h)=(uint32_t)(p_>>32);*(l)=(uint32_t)p_;}while(0)'
echo ok
