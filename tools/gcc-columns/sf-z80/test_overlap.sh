#!/bin/bash
# The driver must refuse a layout whose code and records run into the data at 0xc000.
# Before the guard, 36 KB of library plus 8 KB of records ended at 0x8f66 and the data was written over the
# shift helpers: itof of INT_MIN died at an invalid instruction and the cell reported a failure that was the harness's.
# A bare-ret object stands in for the library, so no cc1 is needed.
here=$(cd "$(dirname "$0")" && pwd); T=$(mktemp -d); trap 'rm -rf "$T"' EXIT
printf '\t.module ok\n\t.area _CODE\n\t.globl ___addsf3\n___addsf3:\n\tret\n' > "$T/ok.s"; sdasz80 -o "$T/ok.rel" "$T/ok.s" || exit 1
# ~5000 records of 12 bytes plus 40 KB of padding code in the library object
python3 - "$T" <<'P'
import sys
t = sys.argv[1]
open(t + "/big.inc", "w").write("NV = 2\nvec:\n" + "\t.word\t0,0,0,0,0,0\n" * 2)
open(t + "/pad.s", "w").write("\t.module pad\n\t.area _CODE\n\t.ds 0xc000\n")
P
sdasz80 -o "$T/pad.rel" "$T/pad.s" || exit 1
o=$(VEC=$T/big.inc OBJS="$T/ok.rel $T/pad.rel" "$here/run.sh" "$T" sf32 add ___addsf3 2 2>&1); rc=$?
case $rc:$o in 1:*"overlap the data"*) ;; *) echo "overlap accepted: rc=$rc: $o"; exit 1;; esac
o=$(VEC=$T/big.inc OBJS="$T/ok.rel" "$here/run.sh" "$T" sf32 add ___addsf3 2 2>&1)
case $o in *"stubfails=0 fails=0"*) echo "test_overlap ok";; *) echo "plain run: $o"; exit 1;; esac
