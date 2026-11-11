#!/bin/bash
# The C calling-convention matrix, run: each convention's caller and callee
# built by BCC (checked in, tests/fixtures/callconv/c/bcc) or llrm-cc, linked
# in all four pairings, and run under dosrun. One output per program:
#   WORK/<K><X>.OUT   K: CF CN PF PN; X: BB LB BL LL (caller, callee; L = llrm)
# A module llrm-cc refuses is never replaced by BCC's: every cell that needs it
# is UNMEASURED (WORK/<cell>.UNMEASURED, no .OUT), WORK/<module>.ERR says why,
# WORK/MODULES.tsv lists each module's result, and the exit status is 1.
# A swap made LL a BCC-against-BCC run that passed, while llrm-cc, given no
# target, refused all 16 modules (`far` is not a keyword to it).
#   tools/callconv/c.sh WORK
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
W="$1"; mkdir -p "$W"
BIN="$(python3 "$ROOT/tools/llrmbin.py" bin)"
DOSRUN="${DOSRUN:-$HOME/scratch/pr-dosbox/src/dosbox-x}"
[[ -x "$DOSRUN" ]] || DOSRUN="$BIN/dosbox-x"
CLANG_M16="$(python3 -I -c 'import sys; sys.path.insert(0, sys.argv[1]); import llrmbin; print(llrmbin.cc_environment().get("LLRM_CLANG_M16", ""))' "$ROOT/tools")"
export LLRM_CLANG_M16="${LLRM_CLANG_M16:-$CLANG_M16}"
BCLIB="${BCLIB:-$HOME/work/other/d32x/toolchains/bcpp31/lib}"
SRC="$ROOT/tests/fixtures/callconv/c"
cp "$SRC"/*.c "$SRC"/*.h "$SRC"/probe.asm "$SRC"/bcc/*.OBJ "$W/"
cd "$W"
"$BIN/jwasm" -q -c -Zg $(python3 "$ROOT/tools/linkrecipe.py" x86-m16 assembler) -FoPROBE.OBJ probe.asm >/dev/null
jobs=""
: > MODULES.tsv
for k in cf cn pf pn; do
    K=${k^^}
    for m in callee caller aggee agger; do
        if "$BIN/llrm-cc" --target=x86_16-dos -mabi=cdecl -c "$k$m.c" -o "$k$m.llrm.obj" 2> "$K${m^^}.ERR"; then
            # The pair must differ: a module byte-equal to BCC's measures nothing.
            if cmp -s "$k$m.llrm.obj" "$K${m^^}.OBJ"; then r=compiled-SAME-AS-BCC; else r=compiled; fi
        else
            rm -f "$k$m.llrm.obj"; r=REFUSED
        fi
        printf '%s\t%s\n' "$K${m^^}" "$r" >> MODULES.tsv
    done
    # llrm's module (the file is absent where llrm-cc refused it), or in the
    # llrm caller's segment where the call is near.
    llrm() { echo $k$1.llrm.obj; }
    near() { if [[ $k == ?n && -f $k$2.llrm.obj ]]; then x=${1:0:3}; echo "$K${x^^}X.OBJ"; else echo "$K${1^^}.OBJ"; fi; }
    link() {
        local exe=$1; shift
        { python3 "$ROOT/tools/linkrecipe.py" x86-m16 format; echo "option quiet"; echo "name $exe.EXE"; echo "file '$BCLIB/C0M.OBJ'"
          for o in ${K}HARN.OBJ PROBE.OBJ "$@"; do echo "file '$o'"; done
          for l in FP87 MATHM CM; do echo "library '$BCLIB/$l.LIB'"; done; } > $exe.lnk
        "$BIN/jwlink" @$exe.lnk > $exe.link 2>&1 || { echo "$exe: link failed"; return 0; }
        jobs+=":ms 20000\nmount w $W\nw:\n$exe > $exe.OUT\n.\n"
    }
    # A cell is linked only when every llrm module it names compiled.
    cell() {
        local exe=$1 mods=$2 m missing=""; shift 2
        for m in $mods; do [[ -f $k$m.llrm.obj ]] || missing+=" $K${m^^}"; done
        if [[ -n $missing ]]; then echo "UNMEASURED: llrm-cc refused$missing" > $exe.UNMEASURED; else link $exe "$@"; fi
    }
    cell ${K}BB "" ${K}CALLER.OBJ ${K}CALLEE.OBJ ${K}AGGER.OBJ ${K}AGGEE.OBJ
    cell ${K}LB "caller agger" $(llrm caller) $(near callee caller) $(llrm agger) $(near aggee agger)
    cell ${K}BL "callee aggee" ${K}CALLER.OBJ $(llrm callee) ${K}AGGER.OBJ $(llrm aggee)
    cell ${K}LL "caller callee agger aggee" $(llrm caller) $(llrm callee) $(llrm agger) $(llrm aggee)
done
printf '[sdl]\nautolock=false\n[dosbox]\nmemsize=16\nstartbanner=false\nquit warning=false\n[cpu]\ncore=normal\ncycles=max\n' > run.conf
printf "$jobs" | DOSRUN_FD=3 SDL_VIDEODRIVER=dummy "$DOSRUN" -nolog -conf run.conf 3>run.events >/dev/null 2>&1
refused=$(grep -c REFUSED MODULES.tsv || true)
echo "llrm compiled $((16 - refused))/16 modules; unmeasured cells: $(ls *.UNMEASURED 2>/dev/null | tr '\n' ' ')" >&2
[[ $refused -eq 0 ]]
