#!/bin/bash
# usage: tests/x86_kernels_ref.sh [NV] -> kernels x_crt_{add,mul,div} (compiler-rt, float typed) and x_sfp_{add,mul,div} (libgcc soft-fp), flattened by the
# pin clang -E, over the same vectors as tests/x86_kernels.sh, in crates/target/llrm-x86-m32/vsgcc/kernels/. Their .out must equal ours (same function).
set -e
cd "$(dirname "$0")/.."
NV=${1:-400}; K=$(cd ../../crates/target/llrm-x86-m32/vsgcc/kernels && pwd); T=$(mktemp -d); trap 'rm -r $T' EXIT
P=/opt/llrm-pin/bin/clang; CRT=${COMPILER_RT:-$HOME/scratch/llvm-src-m16/compiler-rt/lib/builtins}; S=${LIBGCC_SRC:-$HOME/scratch/sf-libgcc-src/libgcc}
cc -O2 -ffp-contract=off tests/gen_vectors.c -o $T/gen && $T/gen $NV > $T/vectors.h
printf '#include <stdio.h>\nvoid report(long v) { printf("%%ld\\n", v); }\n' > $T/rep.c
for op in add mul div; do
  n=x_crt_$op; mkdir -p $K/$n
  { echo 'extern void report(long value);'
    $P -ffreestanding -m32 -E -P -I$CRT $CRT/${op}sf3.c | sed "/Qcomplex/d"
    if [ $op = add ]; then echo 'CRT_FE_ROUND_MODE __fe_getround(void) { return CRT_FE_TONEAREST; }'
    else echo 'int __fe_getround(void) { return 0; }'; fi
    echo 'int __fe_raise_inexact(void) { return 0; }'
    sed 's/uint32_t/unsigned int/' $T/vectors.h
    echo "static float u2f(unsigned int u) { union { unsigned int u; float f; } x; x.u = u; return x.f; } static unsigned int f2u(float f) { union { unsigned int u; float f; } x; x.f = f; return x.u; }"
    echo "long bench_$n(long n) { long i; unsigned int acc = 0; for (i = 0; i < n; ++i) acc += f2u(__${op}sf3(u2f(V[i % NV][0]), u2f(V[i % NV][1]))); return (long)acc; }"
    echo "int main(void) { report(bench_$n($NV)); return 0; }"; } > $K/$n/$n.c
  gcc -m32 -O0 -w $K/$n/$n.c $T/rep.c -o $T/r_$n && $T/r_$n > $K/$n/$n.out
  n=x_sfp_$op; mkdir -p $K/$n
  { echo 'extern void report(long value);'
    gcc -m32 -E -P -I$S/soft-fp -I$S/config/i386 -I$S -I$S/../include $S/soft-fp/${op}sf3.c
    echo 'void __sfp_handle_exceptions(int x) { (void)x; }'
    sed 's/uint32_t/unsigned int/' $T/vectors.h
    echo "static float u2f(unsigned int u) { union { unsigned int u; float f; } x; x.u = u; return x.f; } static unsigned int f2u(float f) { union { unsigned int u; float f; } x; x.f = f; return x.u; }"
    echo "long bench_$n(long n) { long i; unsigned int acc = 0; for (i = 0; i < n; ++i) acc += f2u(__${op}sf3(u2f(V[i % NV][0]), u2f(V[i % NV][1]))); return (long)acc; }"
    echo "int main(void) { report(bench_$n($NV)); return 0; }"; } > $K/$n/$n.c
  gcc -m32 -O0 -w $K/$n/$n.c $T/rep.c -o $T/r_$n && $T/r_$n > $K/$n/$n.out
  echo "$op: ours $(cat $K/x_sf_$op/x_sf_$op.out) crt $(cat $K/x_crt_$op/x_crt_$op.out) softfp $(cat $K/x_sfp_$op/x_sfp_$op.out)"
done
