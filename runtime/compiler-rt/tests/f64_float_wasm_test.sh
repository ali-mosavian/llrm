#!/bin/bash
# f64 float.c on wasm32 with config/wasm: 287 B at -Os and -O2 (main 487). Without SF_CONV_NOINLINE the -O2 inliner copies both helpers: 655 B. Fails above 300 B.
cd "$(dirname "$0")/.."
D=${LLRM_CC_DIR:-$HOME/scratch/mfext-target/release}; T=$(mktemp -d); trap 'rm -r $T' EXIT
for O in Os O2; do
  LLRM_CC_TARGET=wasm32 $D/llrm-cc -$O -c -DSF_INTABI -include config/wasm/config.h -Iinclude lib/fp/f64/float.c -o $T/a.o || exit 2
  n=$(python3 - $T/a.o <<'P'
import sys
b = open(sys.argv[1], 'rb').read(); i = 8
def leb(i):
    r = s = 0
    while True:
        x = b[i]; i += 1; r |= (x & 127) << s; s += 7
        if x < 128: return r, i
while i < len(b):
    sid = b[i]; i += 1; n, i = leb(i)
    if sid == 10: print(n); break
    i += n
P
)
  [ "$n" -le 300 ] || { echo "FAIL: f64 float.c is $n B at wasm32 -$O (limit 300)"; exit 1; }
done
echo "f64_float_wasm ok"
