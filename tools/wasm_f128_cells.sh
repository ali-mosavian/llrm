#!/usr/bin/env bash
# usage: LLRM_CC_DIR=<dir with llrm-cc> LLRM_WASM_LLC=<llrm-wasm-llc> [LLRM_CLANG_WASM=pin clang] tools/wasm_f128_cells.sh [LEVEL...] -> for each long double program of tests/run/c, the pin clang's wasm32 build and
# llrm-wasm-llc's build of the same clang IR, both linked with lib/fp/f128 (pin clang) and a stub `run`, executed by wasm-interp: the hash of what `report` saw must be equal.
# One row per program and level: SAME, DIFF, or `llc FAIL`/`clang FAIL`/`none` (no result: never counted as SAME).
set -u
root=$(cd "$(dirname "$0")/.." && pwd)
clang=${LLRM_CLANG_WASM:-$HOME/scratch/llvm-build-clang-wasm/bin/clang}
llc=${LLRM_WASM_LLC:?path of llrm-wasm-llc}
cc=${LLRM_CC_DIR:?dir holding llrm-cc, to emit the IR}
levels=${*:-O0 O2}
out=$(mktemp -d); trap 'rm -rf "$out"' EXIT
sed -n '/^cat > "\$out\/stub.c"/,/^EOF/p' "$root/tools/wasm_corpus.sh" | sed '1d;$d' > "$out/stub.c"
"$clang" --target=wasm32 -O2 -c "$out/stub.c" -o "$out/stub.o" || exit 2
rt="$root/runtime/compiler-rt"
for f in $(cd "$rt" && sh tests/fp_sources.sh f128) lib/fp/f32/pack.c lib/fp/f64/pack.c lib/int/int128.c lib/int/clzsi2.c lib/int/clzdi2.c; do
  "$clang" --target=wasm32 -O2 -w -ffreestanding -I"$rt/include" -c "$rt/$f" -o "$out/rt_$(echo "$f" | tr / _).o" || exit 2
done
res() { wasm-ld --no-entry --export=run --allow-undefined "$1" "$out/stub.o" "$out"/rt_*.o -o "$1.wasm" 2> /dev/null || { echo none; return; }
  timeout 120 wasm-interp "$1.wasm" -r run 2>&1 | sed -n 's/^run() => i32:\(-*[0-9]*\)$/\1/p' | head -1 | grep . || echo none; }
same=0; total=0
for src in "$root"/tests/run/c/double_to_uint64.c "$root"/tests/run/c/long_double_*.c; do
  b=$(basename "$src" .c)
  for lv in $levels; do
    total=$((total + 1))
    LLRM_CC_TARGET=wasm32 "$cc/llrm-cc" -$lv -emit-llvm -S "$src" -o "$out/$b.ll" 2> /dev/null
    "$llc" -filetype=obj -$lv -o "$out/$b.l.o" "$out/$b.ll" 2> "$out/$b.err" || { printf '%-28s %s llc FAIL: %s\n' "$b" "$lv" "$(head -c 80 "$out/$b.err")"; continue; }
    "$clang" --target=wasm32 -ffreestanding -$lv -c "$src" -o "$out/$b.c.o" 2> /dev/null || { printf '%-28s %s clang FAIL\n' "$b" "$lv"; continue; }
    l=$(res "$out/$b.l.o"); c=$(res "$out/$b.c.o")
    if [ "$l" = "$c" ] && [ "$l" != none ]; then v=SAME; same=$((same + 1)); else v=DIFF; fi
    printf '%-28s %s llrm=%s clang=%s %s\n' "$b" "$lv" "$l" "$c" "$v"
  done
done
echo "$same of $total cells give the pin clang result"
[ "$same" = "$total" ]
