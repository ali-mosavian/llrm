#!/bin/bash
# usage: LLRM_CC_DIR=<dir with llrm-cc> LLRM_CLANG_M16=... tests/scan_size_test.sh
# The first-fit scan of string space walked 184,639 entries on stft_fixed at 27 instructions each (BC's hand loop about 8): it re-read each header through ENTRY_BYTES and wrote it per merge.
# fit+scan were 121 instructions at -O2 m16; one header read per entry and one write per merge make 69.
cd "$(dirname "$0")/.." || exit 2
T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT
LLRM_CC_TARGET=x86_16-wat "${LLRM_CC_DIR:?dir with llrm-cc}/llrm-cc" -march=i386 -O2 -S -I. -Idos/m16 -I../shared nhstutil.c -o $T/s.s 2> /dev/null || exit 2
n=$(awk '/^_fit@3 proc|^_scan@3 proc/ {on=1} on && /^    / {n++} /endp/ {on=0} END {print n+0}' $T/s.s)
[ "$n" -le 80 ] && echo "scan_size ok ($n)" || { echo "FAIL: fit+scan are $n instructions (bound 80)"; exit 1; }
