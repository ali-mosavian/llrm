#!/bin/sh
# DI64_ASM=1 links config/6502/{di64,umod64}.s in place of lib/int (the hand-written divide).
# usage: LLRM_CC=path/to/llrm-cc [LLVM_MOS=sdk/bin] [MOS_RT=mos_rt.s] bench_i64div.sh [OPT] -> sim65 cycles per call and extra bytes of __udivdi3 / __umoddi3 / __divdi3,
# ours (lib/int through llrm-cc 6502-sim65, config/6502) vs llvm-mos (mos-sim-clang, compiler-rt), same 48 operand pairs (magnitudes 1..64 bit, both signs, divisor never 0)
# and the same expected results; fails = mismatches. Cycles: sim65 for ours, mos-sim for llvm-mos; the two count identically (calibrated on one 6502 loop with page-crossing
# indexed loads, 24.0 cycles per iteration in both). +bytes = the entry plus udivmod_w, as linked. Rows that call them: tests/run/c int64_division, shl_of_a_zero_extended_dividend, data_int64.
set -e
cd "$(dirname "$0")"; OPT=${1:-Os}; S=../..; T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT; B=${LLVM_MOS:-$HOME/scratch/fetch-llvm-mos-sdk/x/llvm-mos}/bin; MOS_RT=${MOS_RT:-$S/../../crates/target/llrm-6502/runtime/mos_rt.s}
python3 - $T <<'PY'
import sys, random
random.seed(7); T = sys.argv[1]; M = (1 << 64) - 1
def s(x): return x - (1 << 64) if x >> 63 else x
rows = []
for i in range(48):
    a = random.getrandbits(random.randint(1, 64)); b = random.getrandbits(random.randint(1, 64)) or 1
    if i & 1: a = (-a) & M
    if i & 2: b = (-b) & M
    rows.append((a, b))
def sdiv(a, b):
    x, y = s(a), s(b); q = abs(x) // abs(y); return (q if (x < 0) == (y < 0) else -q) & M
with open(T + "/vec.h", "w") as f:
    f.write("#define NV 48\n")
    f.write("static const unsigned long long A[NV] = {" + ",".join("0x%xull" % a for a, b in rows) + "};\n")
    f.write("static const unsigned long long B[NV] = {" + ",".join("0x%xull" % b for a, b in rows) + "};\n")
    f.write("static const unsigned long long Wu[NV] = {" + ",".join("0x%xull" % (a // b) for a, b in rows) + "};\n")
    f.write("static const unsigned long long Wm[NV] = {" + ",".join("0x%xull" % (a % b) for a, b in rows) + "};\n")
    f.write("static const unsigned long long Ws[NV] = {" + ",".join("0x%xull" % sdiv(a, b) for a, b in rows) + "};\n")
PY
for op in udiv umod sdiv; do
  case $op in udiv) e='A[i] / B[i]'; w=Wu; ent=udivdi3;; umod) e='A[i] % B[i]'; w=Wm; ent=umoddi3;; sdiv) e='(unsigned long long)((long long)A[i] / (long long)B[i])'; w=Ws; ent=divdi3;; esac
  cat > $T/m.c <<E2
#include "vec.h"
int main(void) {
  unsigned fails = 0;
  for (unsigned i = 0; i < NV; i++) {
    unsigned long long r;
#ifdef BASE
    r = $w[i];
#else
    r = $e;
#endif
    if (r != $w[i]) fails++;
  }
  return fails > 255 ? 255 : fails;
}
E2
  for k in base full; do
    d=""; [ $k = base ] && d=-DBASE
    # ours
    srcs="$T/m.c"; [ $k = full ] && srcs="$T/m.c $S/lib/int/udivmod_w.c $S/lib/int/$ent.c"
    [ $k = full ] && [ -n "$DI64_ASM" ] && srcs="$T/m.c"
    : > $T/all.s; rm -f $T/*.s
    for c in $srcs; do $LLRM_CC --target=6502-sim65 -$OPT $d -DSF_INTABI -include $S/config/6502/config.h -I$S/include -I$T -S $c -o $T/$(basename $c .c).s; done
    { grep -h '^rc[0-9]* = ' $T/*.s | sort -u; printf '.export _main\n_main:\n  jsr main\n  rts\n'; for c in $srcs; do grep -v '^rc[0-9]* = ' $T/$(basename $c .c).s || true; done; cat $MOS_RT; } > $T/p.s
    cl65 -t sim6502 -Wa -U -m $T/$k.map -o $T/$k.bin $T/p.s $( [ $k = full ] && [ -n "$DI64_ASM" ] && echo $S/config/6502/$( [ $op = umod ] && echo umod64 || echo di64 ).s ) 2>$T/err || { cat $T/err; exit 1; }
    set +e; sim65 -c -x 2000000000 $T/$k.bin >$T/$k.out 2>/dev/null; eval "f_$k=\$?"; set -e
    eval "c_$k=\$(sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' $T/$k.out | head -1)"
    eval "s_$k=\$(awk '/^(CODE|RODATA) /{print \$4}' $T/$k.map | while read h; do printf '%d\\n' 0x\$h; done | paste -sd+ | bc)"
    # llvm-mos
    $B/mos-sim-clang -$OPT $d -I$T -o $T/m$k $T/m.c 2>$T/err || { cat $T/err; exit 1; }
    set +e; $B/mos-sim --cycles $T/m$k 2>$T/m$k.out >/dev/null; eval "mf_$k=\$?"; set -e
    eval "mc_$k=\$(sed -n 's/^\([0-9][0-9]*\) cycles.*/\1/p' $T/m$k.out | head -1)"
    eval "ms_$k=\$($B/llvm-size $T/m$k.elf | awk 'NR==2{print \$1}')"
  done
  echo "$op -$OPT: ours cycles/call=$(( (c_full - c_base) / 48 )) +bytes=$(( s_full - s_base )) fails=$f_full | llvm-mos cycles/call=$(( (mc_full - mc_base) / 48 )) +bytes=$(( ms_full - ms_base )) fails=$mf_full"
done
