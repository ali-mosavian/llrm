#!/bin/sh
# usage: [CLANG=clang with wasm32, riscv64, aarch64] tests/fp_mul_freestanding.sh [N] -> the f32 and f64 multiplies, divides and int-to-float conversions built for wasm32 (node), riscv64 and aarch64 (qemu-user)
# WITH config/<target>/config.h (the native-multiply hooks) against vectors from the host FPU (tests/fp_mul_vectors.c). A wrong row must be reported, and the wasm32 build
# is also run with a hook that returns a wrong product, which must make the vectors fail (so the hooks are really exercised).
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r $T' EXIT; C=${CLANG:-/usr/bin/clang}; N=${1:-4000}
for t in node wasm-ld ld.lld qemu-riscv64 qemu-aarch64 $C; do command -v $t > /dev/null || { echo "fp_mul_freestanding: $t not found"; exit 2; }; done
cc -O2 -w tests/fp_mul_vectors.c -o $T/gen && $T/gen $N > $T/fp_mul_vec.h || exit 2
cat > $T/run.js <<'J'
const fs = require('fs');
WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {}).then(({ instance }) => { console.log(instance.exports.run()); });
J
srcs="$(sh tests/fp_sources.sh f32 f32_64 f64) $(sh tests/fp_sources.sh int64) lib/int/clzsi2.c lib/int/clzdi2.c tests/fp_mul_freestanding.c"
build() { # triple dir config [extra include]
  mkdir -p $T/$2; rm -f $T/$2/*.o; for f in $srcs; do $C --target=$1 -O2 -w -ffreestanding -include stdint.h -include config/$3/config.h ${4:+-include $4} -Iinclude -I$T "-DSF_NAME(n)=sfb_##n" -c $f -o "$T/$2/$(echo $f | tr / _).o" || exit 2; done; }
n=$(grep -c '^{' $T/fp_mul_vec.h)
build wasm32 w wasm; wasm-ld --no-entry --export=run -o $T/w.wasm $T/w/*.o || exit 2
bad=$(node $T/run.js $T/w.wasm); [ "$bad" = 0 ] || { echo "FAIL: wasm32 got $bad of $n vectors wrong"; exit 1; }
build riscv64-linux-gnu r riscv64; ld.lld -static -o $T/rv $T/r/*.o || exit 2
qemu-riscv64 $T/rv; rc=$?; [ $rc = 0 ] || { echo "FAIL: riscv64 got $rc vectors wrong (capped at 255)"; exit 1; }
build aarch64-linux-gnu a aarch64; ld.lld -static -o $T/aa $T/a/*.o || exit 2
qemu-aarch64 $T/aa; rc=$?; [ $rc = 0 ] || { echo "FAIL: aarch64 got $rc vectors wrong (capped at 255)"; exit 1; }
# a hook with a wrong product must be caught on wasm32
printf '#include <stdint.h>\n#undef SF_MUL32X32\n#define SF_MUL32X32(a, b, h, l) do { uint64_t p_ = (uint64_t)(a) * (b) + 1; *(h) = (uint32_t)(p_ >> 32); *(l) = (uint32_t)p_; } while (0)\n' > $T/bad.h
build wasm32 w wasm $T/bad.h; wasm-ld --no-entry --export=run -o $T/w2.wasm $T/w/*.o || exit 2
[ "$(node $T/run.js $T/w2.wasm)" != 0 ] || { echo "FAIL: a wrong SF_MUL32X32 hook was not detected"; exit 1; }
# a clz hook that is off by one must be caught on riscv64 and aarch64 (denormal normalise path, 1/8 of the operands)
printf '#include <stdint.h>\n#undef SF_CLZ32\n#undef SF_CLZ64\n#define SF_CLZ32(x) ((int)__builtin_clz(x) + 1)\n#define SF_CLZ64(x) ((int)__builtin_clzll(x) + 1)\n' > $T/badclz.h
build riscv64-linux-gnu r riscv64 $T/badclz.h; ld.lld -static -o $T/rv2 $T/r/*.o || exit 2
qemu-riscv64 $T/rv2; [ $? != 0 ] || { echo "FAIL: a wrong riscv64 clz hook was not detected"; exit 1; }
build aarch64-linux-gnu a aarch64 $T/badclz.h; ld.lld -static -o $T/aa2 $T/a/*.o || exit 2
qemu-aarch64 $T/aa2; [ $? != 0 ] || { echo "FAIL: a wrong aarch64 clz hook was not detected"; exit 1; }
# and a wrong row in the vectors
sed -i '$ i {0,0x3f800000ull,0x3f800000ull,0x40000000ull},' $T/fp_mul_vec.h
build wasm32 w wasm; wasm-ld --no-entry --export=run -o $T/w3.wasm $T/w/*.o || exit 2
[ "$(node $T/run.js $T/w3.wasm)" != 0 ] || { echo "FAIL: a wrong row was not detected"; exit 1; }
echo "fp_mul_freestanding ok: $n vectors on wasm32, riscv64, aarch64"
