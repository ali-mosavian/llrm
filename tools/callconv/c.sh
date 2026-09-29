#!/bin/bash
# The C calling-convention matrix, run: each convention's caller and callee
# built by BCC (checked in, tests/fixtures/callconv/c/bcc) or llrm-c, linked
# in all four pairings, and run under dosrun. One output per program:
#   WORK/<K><X>.OUT   K: CF CN PF PN; X: BB LB BL LL (caller, callee; L = llrm)
# A module llrm-c refuses is replaced by BCC's, and WORK/<module>.ERR says why.
#   tools/callconv/c.sh WORK
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
W="$1"; mkdir -p "$W"
BIN="${LLRM_BIN:-$ROOT/target/release}"
DOSRUN="${DOSRUN:-$HOME/scratch/pr-dosbox/src/dosbox-x}"
[[ -x "$DOSRUN" ]] || DOSRUN="$BIN/dosbox-x"
BCLIB="${BCLIB:-$HOME/work/other/d32x/toolchains/bcpp31/lib}"
SRC="$ROOT/tests/fixtures/callconv/c"
cp "$SRC"/*.c "$SRC"/*.h "$SRC"/probe.asm "$SRC"/bcc/*.OBJ "$W/"
cd "$W"
"$BIN/jwasm" -q -c -Zg -omf -FoPROBE.OBJ probe.asm >/dev/null
jobs=""
for k in cf cn pf pn; do
    K=${k^^}
    for m in callee caller aggee agger; do
        "$BIN/llrm-c" "$k$m.c" -o "$k$m.llrm.obj" 2> "$K${m^^}.ERR" || rm -f "$k$m.llrm.obj"
    done
    # llrm's module, or BCC's where llrm refuses it: a callee in the llrm
    # caller's segment where the call is near.
    llrm() { if [[ -f $k$1.llrm.obj ]]; then echo $k$1.llrm.obj; else echo $2; fi; }
    near() { if [[ $k == ?n && -f $k$2.llrm.obj ]]; then x=${1:0:3}; echo "$K${x^^}X.OBJ"; else echo "$K${1^^}.OBJ"; fi; }
    link() {
        local exe=$1; shift
        { echo "format dos"; echo "option quiet"; echo "name $exe.EXE"; echo "file '$BCLIB/C0M.OBJ'"
          for o in ${K}HARN.OBJ PROBE.OBJ "$@"; do echo "file '$o'"; done
          for l in FP87 MATHM CM; do echo "library '$BCLIB/$l.LIB'"; done; } > $exe.lnk
        "$BIN/jwlink" @$exe.lnk > $exe.link 2>&1 || { echo "$exe: link failed"; return 0; }
        jobs+=":ms 20000\nmount w $W\nw:\n$exe > $exe.OUT\n.\n"
    }
    link ${K}BB ${K}CALLER.OBJ ${K}CALLEE.OBJ ${K}AGGER.OBJ ${K}AGGEE.OBJ
    link ${K}LB $(llrm caller ${K}CALLER.OBJ) $(near callee caller) $(llrm agger ${K}AGGER.OBJ) $(near aggee agger)
    link ${K}BL ${K}CALLER.OBJ $(llrm callee ${K}CALLEE.OBJ) ${K}AGGER.OBJ $(llrm aggee ${K}AGGEE.OBJ)
    link ${K}LL $(llrm caller ${K}CALLER.OBJ) $(llrm callee ${K}CALLEE.OBJ) $(llrm agger ${K}AGGER.OBJ) $(llrm aggee ${K}AGGEE.OBJ)
done
printf '[sdl]\nautolock=false\n[dosbox]\nmemsize=16\nstartbanner=false\nquit warning=false\n[cpu]\ncore=normal\ncycles=max\n' > run.conf
printf "$jobs" | DOSRUN_FD=3 SDL_VIDEODRIVER=dummy "$DOSRUN" -nolog -conf run.conf 3>run.events >/dev/null 2>&1
