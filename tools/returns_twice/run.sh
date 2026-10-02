#!/bin/bash
# A setjmp/longjmp program built by llrm-c and run under dosrun, at -O2 and -Os:
# the second return from setjmp must find every local nothing changed.
#   tools/returns_twice/run.sh WORK
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
W="$1"; mkdir -p "$W"
BIN="${LLRM_BIN:-$ROOT/target/release}"
DOSRUN="${DOSRUN:-$HOME/scratch/pr-dosbox/src/dosbox-x}"
[[ -x "$DOSRUN" ]] || DOSRUN="$BIN/dosbox-x"
BC="${BC:-$HOME/work/other/d32x/toolchains/bcpp31}"
EXPECT='got=5 a=10 b=17 c=24 d=31 changed=99 thrown=1'
cp "$ROOT/tests/fixtures/returns_twice/twice.c" "$W/"
cd "$W"
# Borland's headers are upper case, and the front end looks for lower case.
rm -rf inc && mkdir inc
for h in "$BC"/include/*.[Hh]; do ln -s "$h" "inc/$(basename "$h" | tr 'A-Z' 'a-z')"; done
jobs=""
for level in O2 Os; do
    dos="$(tr a-z A-Z <<< "$level")"
    "$BIN/llrm-c" -$level -I inc twice.c -o "twice$level.obj"
    { echo "format dos"; echo "option quiet"; echo "name T$dos.EXE"; echo "file '$BC/lib/C0M.OBJ'"
      echo "file 'twice$level.obj'"; for l in MATHM CM; do echo "library '$BC/lib/$l.LIB'"; done; } > "$level.lnk"
    "$BIN/jwlink" "@$level.lnk" > "$level.link" 2>&1
    jobs+=":ms 20000\nmount w $W\nw:\nT$dos > T$dos.OUT\n.\n"
done
printf '[sdl]\nautolock=false\n[dosbox]\nmemsize=16\nstartbanner=false\nquit warning=false\n[cpu]\ncore=normal\ncycles=max\n' > run.conf
printf "$jobs" | DOSRUN_FD=3 SDL_VIDEODRIVER=dummy "$DOSRUN" -nolog -conf run.conf 3>run.events >/dev/null 2>&1
status=0
for level in O2 Os; do
    dos="$(tr a-z A-Z <<< "$level")"
    got="$(tr -d '\r' < T$dos.OUT 2>/dev/null | head -1)"
    if [[ "$got" == "$EXPECT" ]]; then echo "$level: ok"; else echo "$level: got '$got', want '$EXPECT'"; status=1; fi
done
exit $status
