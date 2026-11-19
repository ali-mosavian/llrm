#!/bin/bash
# usage: LLRM_CC_DIR=dir tools/softfloat_cells/m68k/bytes_test.sh
# bytes.sh read .text only and subtracted a null program that lacked the probe's own code: every row carried 12 B (68000) of probe, and libgcc's data
# was dropped (68000 -Os __udivdi3 libgcc 1280 B read, 1524 B with data; ours __ashldi3 90 B read, 78 B real).
cd "$(dirname "$0")" || exit 2
out=$(ENTRIES="__ashldi3 __udivdi3" bash ./bytes.sh Os) || exit 2; bad=0
row() { echo "$out" | awk -v k="68000 -Os $1:" 'index($0,k)==1'; }
num() { case $1 in ''|*[!0-9]*) echo "FAIL: no number for $2 in: $out"; bad=1; return 1;; esac; }
set -- $(row __ashldi3); num "$5" ashldi3 && { [ "$5" -le 80 ] || { echo "FAIL: 68000 -Os __ashldi3 ours $5 B carries the probe's own code (want <= 80)"; bad=1; }; }
set -- $(row __udivdi3); num "$7" udivdi3 && { [ "$7" -ge 1500 ] || { echo "FAIL: 68000 -Os __udivdi3 libgcc $7 B leaves out its data (want >= 1500)"; bad=1; }; }
[ $bad = 0 ] && echo "m68k bytes ok"; exit $bad
