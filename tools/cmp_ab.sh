#!/bin/bash
# cmp_ab.sh OUT A B prog:level... — A/B of two compiler dists (dirs holding llrm-cc) on bench programs, x86-64.
#   per row: text bytes, instructions:u and executed loads/stores (instructions exact, loads/stores approximate),
#   cycles:u (min of 15, interleaved, 4 text placements) and a CONTROL column (A's own binary, copied, run alongside).
#   Control off by more than 1% in any placement => NOISY: judge the row by instructions + bytes, not cycles.
#   Host cycles move +-8% under load; the instruction counter does not.
#   prog:LA/LB compiles A at LA and B at LB (a perturbed variant).
#   cmp_ab.sh --selftest DIR prog:level   A against itself (instructions equal; control within 1% or flagged NOISY);
#   the same with the control skewed 5% (must be flagged NOISY); A at level against A at -O1 (instructions must differ
#   by >= 1%). CMP_AB_BREAK=1 makes B the binary of A, as a broken instrument would: the last check must then fail.
# Env: REPO (tree with bench/ and runtime/), TOOLS (dir with instrcount_driver.c), CORE (cpu to pin, default 6).
: "${REPO:?set REPO}" "${TOOLS:?set TOOLS}"; CORE=${CORE:-6}
R=$REPO/runtime/shared/posix
if [ "$1" = --selftest ]; then
  shift; dist=$1; prog=$2; self=$(readlink -f "$0")
  fail(){ echo "SELFTEST FAIL: $1"; exit 1; }
  row(){ grep -v '^#\|^done' "$1" | head -1; }
  pair(){ echo "$1" | sed 's/.* ins \([0-9]*\)->\([0-9]*\) .*/\1 \2/'; }
  out=$(mktemp -d)
  "$self" $out/same "$dist" "$dist" "$prog"; r=$(row $out/same); echo "$r" | cut -c1-200
  echo "$r" | grep -q compile-fail && fail compile
  set -- $(pair "$r"); d=$(( $1 > $2 ? $1 - $2 : $2 - $1 ))
  [ $((d * 100000)) -gt "$1" ] && fail "instructions of one binary differ by $d"
  echo "$r" | grep -qE "control_max_permille=([0-9]|10) ok|NOISY" || fail "control neither within 1% nor flagged"
  CMP_AB_CTL_SKEW=5 "$self" $out/skew "$dist" "$dist" "$prog"
  row $out/skew | grep -q NOISY || fail "a control skewed 5% was not flagged NOISY"
  "$self" $out/pert "$dist" "$dist" "${prog%%:*}:${prog##*:}/O1"; r=$(row $out/pert)
  set -- $(pair "$r"); d=$(( $1 > $2 ? $1 - $2 : $2 - $1 ))
  [ $((d * 100)) -lt "$1" ] && fail "a variant at -O1 did not read as a difference ($1 -> $2)"
  echo "SELFTEST ok"; exit 0
fi
OUT=$1; A=$2; B=$3; shift 3
t=$(mktemp -d); trap 'rm -rf $t' EXIT; cd $t
for n in 0 16 32 48; do printf '.text\n.p2align 6\n.space %d,0x90\n' $n > pad$n.s; gcc -c pad$n.s -o pad$n.o 2>/dev/null; done
echo "# $(date -u +%FT%TZ) A=$A($(md5sum <$A/llrm-cc|cut -c1-8)) B=$B($(md5sum <$B/llrm-cc|cut -c1-8)) load $(cut -d' ' -f1-3 /proc/loadavg)" > $OUT
meas(){ setarch -R taskset -c $CORE perf stat -x, -e cycles:u,instructions:u,ls_dispatch.ld_dispatch:u,ls_dispatch.store_dispatch:u $1 5000 2>&1 >/dev/null | awk -F, '$3 ~ /^cycles/ {c=$1} $3 ~ /^instructions/ {i=$1} $3 ~ /ld_dispatch/ {l=$1} $3 ~ /store_dispatch/ {s=$1} END{print c, i, l, s}'; }
for pl in "$@"; do p=${pl%%:*}; L=${pl##*:}; LA=${L%%/*}; LB=${L##*/}; ctlmax=0; pm=
 for k in a b; do d=$A; lv=$LA; [ $k = b ] && { d=$B; lv=$LB; }
  (cd $REPO/bench/$p; LLRM_CC_TARGET=x86_64-linux $d/llrm-cc -m64 -$lv -c -Dmain=bench_main_ -I $REPO/bench -I $R -o $t/o.$k.o $p.c) >/dev/null 2>&1 || { echo "$p $L compile-fail $k" >> $OUT; continue 2; }
 done
 bytesA=$(size $t/o.a.o | awk 'NR==2{print $1}'); bytesB=$(size $t/o.b.o | awk 'NR==2{print $1}')
 for pd in 0 16 32 48; do
  for k in a b; do gcc -static -no-pie -o $t/x.$k $t/pad$pd.o $t/o.$k.o $TOOLS/instrcount_driver.c $R/os.c -lm >/dev/null 2>&1; done
  cp $t/x.a $t/x.ctl; [ -n "$CMP_AB_BREAK" ] && cp $t/x.a $t/x.b
  bA=999999999999; bB=$bA; bC=$bA
  for r in $(seq 1 15); do
   read c i l st < <(meas $t/x.a); [ "$c" -lt "$bA" ] && bA=$c; insA=$i; ldA=$l; stA=$st
   read c i l st < <(meas $t/x.b); [ "$c" -lt "$bB" ] && bB=$c; insB=$i; ldB=$l; stB=$st
   read c i l st < <(meas $t/x.ctl); c=$((c * (100 + ${CMP_AB_CTL_SKEW:-0}) / 100)); [ "$c" -lt "$bC" ] && bC=$c
  done
  pm="$pm $bA,$bB,$bC"; d=$(( (bC>bA ? bC-bA : bA-bC) * 1000 / bA )); [ "$d" -gt "$ctlmax" ] && ctlmax=$d
 done
 flag=ok; [ "$ctlmax" -gt 10 ] && flag=NOISY
 echo "$p $L bytes $bytesA->$bytesB ins $insA->$insB loads $ldA->$ldB stores $stA->$stB control_max_permille=$ctlmax $flag placements(A,B,ctl cycles):$pm" >> $OUT
done; echo done >> $OUT
