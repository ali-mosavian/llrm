#!/bin/sh
# usage: [CLANG=clang with the AArch64 target] tests/f128_aarch64.sh [N] -> lib/fp/f128 built for aarch64-linux with SF_QUAD_LONGDOUBLE (the tf libcalls take the IEEE quad in
# vector registers), linked to tests/host_f128.c and run under qemu-aarch64 against libgcc's own tf routines. The bits ABI (the default, right for wasm32) must fail
# there: it passes the operands in other registers (33069 of 65349 cases at N=2000).
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r $T' EXIT; C=${CLANG:-/usr/bin/clang}
for t in qemu-aarch64 aarch64-linux-gnu-gcc; do command -v $t > /dev/null || { echo "f128_aarch64: $t not found"; exit 2; }; done
build() { # mode flag
  mkdir $T/$1; for f in $(sh tests/fp_sources.sh f128) lib/fp/f32/pack.c lib/fp/f64/pack.c; do
    $C --target=aarch64-linux-gnu -O2 -w -ffreestanding $2 -Iinclude '-DSF_NAME(n)=sfb_##n' -c $f -o $T/$1/$(echo $f | tr / _).o || exit 2; done
  aarch64-linux-gnu-gcc -O2 -w -fno-builtin -ffp-contract=off -D__float128=_Float128 -DSF_QUAD_FLOAT128 -Iinclude '-DSF_NAME(n)=sfb_##n' tests/host_f128.c $T/$1/*.o -o $T/$1/t -static || exit 2
}
build ld -DSF_QUAD_LONGDOUBLE; build bits ""
timeout 300 qemu-aarch64 $T/ld/t ${1:-20000} | tail -1 | grep -qE '^n=[0-9]+ fails=0$' || { echo "FAIL: the long double build differs from libgcc's tf routines"; exit 1; }
timeout 300 qemu-aarch64 $T/bits/t 2000 | tail -1 | grep -qE 'fails=0$' && { echo "FAIL: the bits ABI passed on aarch64: the test does not discriminate"; exit 1; }
echo "f128_aarch64 ok"
