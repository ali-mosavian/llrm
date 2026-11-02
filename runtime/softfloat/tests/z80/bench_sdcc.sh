#!/bin/sh
# usage: tests/z80/bench_sdcc.sh [ops]   (fadd fsub fmul fdiv citof cutof cftoi cftou fslt fsgt fseq)
# The same vectors through SDCC's own float routines (sdcc -mz80, default convention) on ucsim_z80: extra code bytes (_CODE + _HOME,
# the op build minus a plain copy), mean cycles per call (minus the copy), and mismatches against the host (SDCC has no NaN or denormals).
set -e
cd "$(dirname "$0")"; OPS=${*:-fadd fsub fmul fdiv citof cutof cftoi cftou}; T=$(mktemp -d)
printf '\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n' > $T/crt0.s
sdasz80 -o $T/crt0.rel $T/crt0.s
csize() { n=0; for h in $(awk '($1=="_CODE" || $1=="_HOME") && $3 ~ /^0000/ {print $3}' "$1"); do n=$((n + 0x$h)); done; echo $n; }
run() { printf 'run\nstate\ndump rom 0x8f00 0x8f01\nquit\n' | ucsim_z80 -t Z80 -q -w $1 -c - 2>&1; }
for op in $OPS; do
  case $op in fslt|fsgt|fseq) python3 gen_cmp_bool.py ${op#f} > $T/vec.inc;; f*|c*) python3 gen_f32.py ${op#?} > $T/vec.inc;; *) python3 gen.py $op > $T/vec.inc;; esac
  python3 - $T/vec.inc $op > $T/d.c <<'P'
import re, sys
w = [int(x, 16) for l in open(sys.argv[1]) if l.strip().startswith('.word') for x in l.split('\t')[2].split(',')]
v = [(w[i] | w[i+1] << 16, w[i+2] | w[i+3] << 16, w[i+4] | w[i+5] << 16) for i in range(0, len(w), 6)]
body = {'fadd': 'x.f + y.f', 'fsub': 'x.f - y.f', 'fmul': 'x.f * y.f', 'fdiv': 'x.f / y.f', 'fslt': 'x.f < y.f', 'fsgt': 'x.f > y.f', 'fseq': 'x.f == y.f',
        'citof': '(float)(long)x.u', 'cutof': '(float)x.u', 'cftoi': None, 'cftou': None,
        'mul': 'x.u * y.u', 'udiv': 'y.u ? x.u / y.u : 0xffffffffUL', 'umod': 'y.u ? x.u % y.u : x.u', 'div': 'x.s / y.s', 'mod': 'x.s % y.s'}[sys.argv[2]]
print("typedef union { float f; unsigned long u; long s; } U;")
for n, k in (('A', 0), ('B', 1), ('E', 2)):
    print("static const unsigned long %s[] = {%s};" % (n, ','.join('0x%xUL' % t[k] for t in v)))
print("#ifdef BASE\n#define OP(x, y, r) r.u = E[i]\n#else")
if sys.argv[2] == 'cftoi': print("#define OP(x, y, r) r.s = (long)x.f")
elif sys.argv[2] == 'cftou': print("#define OP(x, y, r) r.u = (unsigned long)x.f")
elif sys.argv[2] == 'citof': print("#define OP(x, y, r) r.f = (float)x.s")
elif sys.argv[2] in ("mul", "udiv", "umod"): print("#define OP(x, y, r) r.u = %s" % body)
elif sys.argv[2] in ("div", "mod"): print("#define OP(x, y, r) r.s = %s" % body)
else: print("#define OP(x, y, r) r.f = %s" % body)
print("#endif")
print("void main(void) { unsigned fails = 0; unsigned i; for (i = 0; i < %d; i++) { U x, y, r; x.u = A[i]; y.u = B[i]; OP(x, y, r); if (r.u != E[i]) fails++; } *(volatile unsigned *)0x8f00 = fails; }" % len(v))
P
  for k in base full; do
    d=""; [ $k = base ] && d=-DBASE
    sdcc -mz80 $d --no-std-crt0 --code-loc 0x100 --data-loc 0x9000 -Wl-b_HOME=0x4000 -o $T/$k.ihx $T/crt0.rel $T/d.c >/dev/null 2>$T/err || { cat $T/err; exit 1; }
    eval "o_$k=\$(run $T/$k.ihx)"
    eval "c_$k=\$(echo \"\$o_$k\" | sed -n 's/.*(\\([0-9]*\\) clks).*/\\1/p' | head -1)"
    eval "s_$k=\$(csize $T/$k.map)"
  done
  fb=$(echo "$o_base" | sed -n 's/^0x8f00 *\([0-9a-f][0-9a-f]\) *\([0-9a-f][0-9a-f]\).*/\2\1/p' | head -1)
  [ "${fb:-ffff}" = 0000 ] || { echo "$op: the base run must pass its records, got $fb"; exit 1; }
  f=$(echo "$o_full" | sed -n 's/^0x8f00 *\([0-9a-f][0-9a-f]\) *\([0-9a-f][0-9a-f]\).*/\2\1/p' | head -1)
  nv=$(sed -n 's/^NV = //p' $T/vec.inc)
  echo "$op sdcc: +bytes=$(( s_full - s_base )) fails=$((0x${f:-ffff})) cycles=$(( (c_full-c_base)/nv ))"
done
rm -r $T
