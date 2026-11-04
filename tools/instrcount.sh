#!/bin/bash
# instrcount.sh EXE [REPS=20] : exact user instructions per repetition of an
# EXE built with instrcount_driver.c. round((count(REPS+1) - count(1)) / REPS), with
# ASLR off and perf's instructions:u, so the start-up cancels.
exe=$1; n=${2:-20}
count() { setarch -R perf stat -x, -e instructions:u "$exe" "$1" 2>&1 >/dev/null | awk -F, '$3 ~ /instr/ {print $1}'; }
a=$(count 1); b=$(count $((n + 1)))
echo $(( (b - a + n / 2) / n ))
