#!/bin/sh
# usage: [CLANG=clang with wasm32+riscv64] tests/f128_freestanding.sh [N] -> lib/fp/f128 (bits ABI, the one the wasm32 and riscv64 tf libcalls use) built freestanding for
# wasm32 (run in node) and riscv64-linux (run under qemu-riscv64), add/sub/mul/div against vectors made on the host from libgcc's __float128 (tests/f128_vectors.c).
# A row with a wrong result must be reported, so a driver that checks nothing cannot pass.
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r $T' EXIT; C=${CLANG:-/usr/bin/clang}; N=${1:-3000}
for t in node wasm-ld ld.lld qemu-riscv64 $C; do command -v $t > /dev/null || { echo "f128_freestanding: $t not found"; exit 2; }; done
cc -O2 -w tests/f128_vectors.c -o $T/gen && $T/gen $N > $T/f128_vec.h || exit 2
cat > $T/run.js <<'J'
const fs = require('fs');
WebAssembly.instantiate(fs.readFileSync(process.argv[2]), {}).then(({ instance }) => { console.log(instance.exports.run()); });
J
srcs="$(sh tests/fp_sources.sh f128) lib/fp/f32/pack.c lib/fp/f64/pack.c lib/int/int128.c tests/f128_freestanding.c"
build() { # triple dir config
  mkdir -p $T/$2; for f in $srcs; do $C --target=$1 -O2 -w -ffreestanding -include stdint.h -include config/$3/config.h -Iinclude -I$T "-DSF_NAME(n)=sfb_##n" -c $f -o "$T/$2/$(echo $f | tr / _).o" || exit 2; done; }
build wasm32 w wasm; wasm-ld --no-entry --export=run -o $T/w.wasm $T/w/*.o || exit 2
build riscv64-linux-gnu r riscv64; ld.lld -static -o $T/rv $T/r/*.o || exit 2
n=$(grep -c '^{' $T/f128_vec.h)
bad=$(node $T/run.js $T/w.wasm); [ "$bad" = 0 ] || { echo "FAIL: wasm32 got $bad of $n vectors wrong"; exit 1; }
qemu-riscv64 $T/rv; rc=$?; [ $rc = 0 ] || { echo "FAIL: riscv64 got $rc vectors wrong (capped at 255)"; exit 1; }
# add a row with a wrong result and rebuild wasm: it must be caught
sed -i '$ i {0,{1,0},{1,0},{5,5}},' $T/f128_vec.h
build wasm32 w wasm; wasm-ld --no-entry --export=run -o $T/w2.wasm $T/w/*.o || exit 2
[ "$(node $T/run.js $T/w2.wasm)" != 0 ] || { echo "FAIL: a wrong row was not detected"; exit 1; }
echo "f128_freestanding ok: $n vectors on wasm32 and riscv64"
