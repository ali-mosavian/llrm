#!/bin/bash
# Compile C files with BCC 3.1 (medium model) under one dosrun boot.
#   tools/callconv/bcc.sh OUTDIR 'FILE.c FLAGS...' ...
# Each argument is one bcc invocation's file and extra flags; objects,
# -S listings (when the flags say -S) and BCC's messages land in OUTDIR.
set -euo pipefail
OUT="$1"; shift
TOOLCHAINS="${TOOLCHAINS:-$HOME/work/other/d32x/toolchains}"
DOSRUN="${DOSRUN:-$HOME/scratch/pr-dosbox/src/dosbox-x}"
CCDIR="${CCDIR:-bcpp31}"  # tc201 and tcpp30 have a TCC.EXE with BCC's switches; their jobs pass every switch themselves
CCEXE="${CCEXE:-bcc}"
CCROOT="${CCROOT:-$TOOLCHAINS}"
mkdir -p "$OUT"
printf '[sdl]\nautolock=false\n[dosbox]\nmemsize=16\nstartbanner=false\nquit warning=false\n[cpu]\ncore=dynamic\ncycles=max\n' > "$OUT/job.conf"
{
    printf ':ms 3600000\nmount w %s\nmount b %s/%s\npath b:\\bin\nw:\n' "$OUT" "$CCROOT" "$CCDIR"
    for one in "$@"; do
        set -- $one
        file=$1; shift
        base=$(basename "$file" .c)
        case " $* " in *" -O"*) opt="";; *) opt="-Ox";; esac  # -O1 for size replaces -Ox, it does not follow it
        if [ "$CCEXE" = bcc ]; then
            echo "bcc -c -mm -3 -f87 $opt -IB:\\INCLUDE $* $base.c > $base.msg"
        else
            echo "$CCEXE -c -mm -f87 -IB:\\INCLUDE $* $base.c > $base.msg"
        fi
    done
    printf '.\n'
} | DOSRUN_FD=3 SDL_VIDEODRIVER=dummy "$DOSRUN" -nolog -conf "$OUT/job.conf" 3>"$OUT/events.txt" >/dev/null 2>&1
grep -l "Error\|Fatal" "$OUT"/*.MSG 2>/dev/null && exit 1 || true
