#!/bin/bash
# cmp_ab.sh OUT A B prog:level... — A/B of two compiler dists (dirs holding llrm-cc) on bench programs, x86-64.
#   per row: text bytes, instructions:u and executed loads/stores (exact), cycles:u (min of 15, interleaved, 4 text placements)
#   and a CONTROL column (A's own binary, copied, run alongside). Control off by more than 1% in any placement => NOISY:
#   judge the row by instructions + bytes, not cycles. Host cycles move +-8% under load; the counters do not.
#   cmp_ab.sh --selftest DIR prog:level   A against itself: instructions must be equal, control within 1% or flagged NOISY.
# Env: REPO (tree with bench/ and runtime/), TOOLS (dir with instrcount_driver.c), CORE (cpu to pin, default 6).
: "${REPO:?set REPO}" "${TOOLS:?set TOOLS}"; CORE=${CORE:-6}
R=$REPO/runtime/shared/posix
self=0; [ "$1" = --selftest ] && { self=1; shift; }
if [ $self = 1 ]; then OUT=$(mktemp); A=$1; B=$1; shift; set -- "$@"; else OUT=$1; A=$2; B=$3; shift 3; fi
t=$(mktemp -d); trap 'rm -rf $t' EXIT; cd $t
for n in 0 16 32 48; do printf '.text\n.p2align 6\n.space %d,0x90\n' $n > pad$n.s; gcc -c pad$n.s -o pad$n.o 2>/dev/null; done
echo "# $(date -u +%FT%TZ) A=$A($(md5sum <$A/llrm-cc|cut -c1-8)) B=$B($(md5sum <$B/llrm-cc|cut -c1-8)) load $(cut -d' ' -f1-3 /proc/loadavg)" > $OUT
meas(){ setarch -R taskset -c $CORE perf stat -x, -e cycles:u,instructions:u,ls_dispatch.ld_dispatch:u,ls_dispatch.store_dispatch:u $1 5000 2>&1 >/dev/null | awk -F, '$3 ~ /^cycles/ {c=$1} $3 ~ /^instructions/ {i=$1} $3 ~ /ld_dispatch/ {l=$1} $3 ~ /store_dispatch/ {s=$1} END{print c, i, l, s}'; }
for pl in "$@"; do p=${pl%%:*}; L=${pl##*:}; ctlmax=0
 for k in a b; do d=$A; [ $k = b ] && d=$B; (cd $REPO/bench/$p; LLRM_CC_TARGET=x86_64-linux $d/llrm-cc -m64 -$L -c -Dmain=bench_main_ -I $REPO/bench -I $R -o $t/o.$k.o $p.c) >/dev/null 2>&1 || { echo "$p $L compile-fail $k" >> $OUT; continue 2; }; done
 bytesA=$(size $t/o.a.o | awk 'NR==2{print $1}'); bytesB=$(size $t/o.b.o | awk 'NR==2{print $1}'); pm=
 for pd in 0 16 32 48; do
  for k in a b; do gcc -static -no-pie -o $t/x.$k $t/pad$pd.o $t/o.$k.o $TOOLS/instrcount_driver.c $R/os.c -lm >/dev/null 2>&1; done; cp $t/x.a $t/x.ctl
  bA=999999999999; bB=$bA; bC=$bA
  for r in $(seq 1 15); do
   read c i l st < <(meas $t/x.a); [ "$c" -lt "$bA" ] && bA=$c; insA=$i; ldA=$l; stA=$st
   read c i l st < <(meas $t/x.b); [ "$c" -lt "$bB" ] && bB=$c; insB=$i; ldB=$l; stB=$st
   read c i l st < <(meas $t/x.ctl); [ "$c" -lt "$bC" ] && bC=$c
  done
  pm="$pm $bA,$bB,$bC"; d=$(( (bC>bA ? bC-bA : bA-bC) * 1000 / bA )); [ "$d" -gt "$ctlmax" ] && ctlmax=$d
 done
 flag=ok; [ "$ctlmax" -gt 10 ] && flag=NOISY
 echo "$p $L bytes $bytesA->$bytesB ins $insA->$insB loads $ldA->$ldB stores $stA->$stB control_max_permille=$ctlmax $flag placements(A,B,ctl cycles):$pm" >> $OUT
done; echo done >> $OUT
if [ $self = 1 ]; then
  row=$(grep -v '^#\|^done' $OUT | head -1); echo "$row" | cut -c1-200
  echo "$row" | grep -q "compile-fail" && { echo "SELFTEST FAIL: compile"; exit 1; }
  a=$(echo "$row" | sed 's/.* ins \([0-9]*\)->\([0-9]*\) .*/\1 \2/'); set -- $a
  d=$(( $1 > $2 ? $1 - $2 : $2 - $1 ))
  [ $((d * 100000)) -gt $1 ] && { echo "SELFTEST FAIL: instructions of one binary differ by $d"; exit 1; }
  echo "$row" | grep -qE "control_max_permille=([0-9]|10) ok|NOISY" || { echo "SELFTEST FAIL: control neither within 1% nor flagged"; exit 1; }
  echo "SELFTEST ok"
fi
