#!/bin/bash
# usage: tests/copy_prologue_test.sh   (jwasm on PATH)
# B$COPY saved bx and cx, which this ABI treats as scratch (it destroys bx and keeps si and di): 4 extra push/pop per call, 130,817 calls on stft_fixed.
# Only si, di and es are saved (3 generated pushes + the explicit `push ds`).
cd "$(dirname "$0")/../dos/m16" || exit 2
T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT
jwasm -nologo -Sg -c -Fl=$T/b.lst -Fo$T/b.obj -I. block.asm > /dev/null || exit 2
n=$(awk '/B\$COPY +proc/ {on=1} on && /push/ {c++} /B\$COPY +endp/ {on=0} END {print c+0}' $T/b.lst)
[ "$n" -le 4 ] && echo "copy_prologue ok ($n pushes)" || { echo "FAIL: B\$COPY saves $n registers (bound 4)"; exit 1; }
