#!/bin/bash
# ptour.sh PROG LEVEL [DIST]: the pin's llc on OUR final IR (llrm-cc's last mid-end stage) against our object,
# kernel bytes of both. Same IR on both sides: the pin's own clang IR through our llc re-runs our mid-end.
# Env: PIN_LLC (assert build llc), CELL_SRC (dir of PROG.c), WORK (scratch, on disk).
P=$1; L=$2; D=${3:-$DIST}; H=$(cd "$(dirname "$0")" && pwd); B=$(cd "$H/../.." && pwd)
PIN=${PIN_LLC:?PIN_LLC}; SRC=${CELL_SRC:-$B/bench/$P}; W=${WORK:-$HOME/scratch/stg}/$P.$L; rm -rf "$W"; mkdir -p "$W"
N=$(python3 "$H/flagset.py" --neutral); NC=${N/-fzero-init-padding-bits=standard/}
T="--target=aarch64-unknown-linux-gnu -I $B/runtime/shared/posix -Dfar= -Dnear= -Dhuge= -D__huge= -D__386__=1 -include $H/host_shim.h"
LLRM_ARM64_SYSROOT=${LLRM_ARM64_SYSROOT:-/usr/aarch64-linux-gnu} LLRM_MIR_STAGES=$W/d "$D/llrm-cc" $T $NC -O$L -ffreestanding -c "$SRC/$P.c" -o "$W/ours.o" 2>/dev/null || { echo "$P $L cc-failed"; exit 0; }
F=$(ls "$W"/d/*.ll | sort | tail -1)
python3 "$H/ptour_ir.py" "$F" "$W/ours.ll" || { echo "$P $L refused"; exit 0; }
O=$L; [ "$L" = s ] && O=2
"$PIN" -mtriple=aarch64-unknown-linux-gnu -relocation-model=pic -O$O -filetype=obj -o "$W/pin.o" "$W/ours.ll" 2>/dev/null || { echo "$P $L pin-failed"; exit 0; }
python3 - "$P" "$L" "$W" "$H" <<'PY'
import sys
sys.path.insert(0, sys.argv[4]); import matrix_kb
p, l, w = sys.argv[1:4]
print(p, l, 'pin=%d ours=%d' % (matrix_kb.kernel_bytes(w + '/pin.o'), matrix_kb.kernel_bytes(w + '/ours.o')))
PY
