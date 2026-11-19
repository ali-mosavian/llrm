#!/bin/sh
# usage: [CLANG=clang with aarch64, riscv64, wasm32] tests/mul_hook_size_test.sh -> the native-multiply hooks of config/{aarch64,riscv64,wasm} keep the soft-float multiplies small at -Os.
# Without them the generic 16x16 partial products inline: f128 mul 2276 B (aarch64), 2912 (riscv64), 2528 (wasm32, code section) against 1336, 1246, 1460 with them.
# Each bound must hold with the config and be exceeded without it, so the check cannot pass on a missing config.
cd "$(dirname "$0")/.." || exit 2; T=$(mktemp -d); trap 'rm -r $T' EXIT; C=${CLANG:-/usr/bin/clang}; bad=0
cat > $T/wt.py <<'P'
import sys
tot = 0
for path in sys.argv[1:]:
    b = open(path, 'rb').read(); i = 8
    def leb(i):
        r = s = 0
        while True:
            x = b[i]; i += 1; r |= (x & 127) << s; s += 7
            if x < 128: return r, i
    while i < len(b):
        sid = b[i]; i += 1; n, i = leb(i)
        if sid == 10: tot += n; break
        i += n
print(tot)
P
size_of() { # triple config lib/fp-dir mulfile
  if [ "$2" = none ]; then inc="-include stdint.h"; else inc="-include stdint.h -include config/$2/config.h"; fi
  $C --target=$1 -Os -w -ffreestanding $inc -Iinclude -c lib/fp/$3/$4.c -o $T/a.o && $C --target=$1 -Os -w -ffreestanding $inc -Iinclude -c lib/fp/$3/pack.c -o $T/b.o || { echo 0; return; }
  case $1 in wasm32) python3 $T/wt.py $T/a.o $T/b.o;; *) size -A $T/a.o $T/b.o | awk '$1==".text"{t+=$2} END{print t+0}';; esac; }
check() { # triple config dir file bound
  w=$(size_of $1 $2 $3 $4); n=$(size_of $1 none $3 $4)
  [ "$w" -gt 0 ] && [ "$w" -le "$5" ] || { echo "FAIL: $1 $3/$4 with config/$2 is $w B (bound $5)"; bad=1; }
  [ "$n" -gt "$5" ] || { echo "FAIL: $1 $3/$4 without the config is $n B, not above $5: the bound does not discriminate"; bad=1; }; }
check aarch64-linux-gnu aarch64 f128 mul 1450
check riscv64-linux-gnu riscv64 f128 mul 1350
check wasm32 wasm f128 mul 1550
check aarch64-linux-gnu aarch64 f64 mul 950
check riscv64-linux-gnu riscv64 f64 mul 780
check wasm32 wasm f64 mul 800
[ $bad = 0 ] && echo "mul_hook_size ok"; exit $bad
