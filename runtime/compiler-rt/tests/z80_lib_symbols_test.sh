#!/bin/bash
# The library check read only `.globl`: a library whose compiled members define their functions as `name::`
# (the llc's form) failed with 22 symbols "not defined" (11-07) and the f32 A/B had to be checked by hand.
set -e
cd "$(dirname "$0")"; T=$(mktemp -d); trap 'rm -r $T' EXIT
syms="__adddf3 __subdf3 __muldf3 __divdf3 __eqdf2 __nedf2 __ltdf2 __ledf2 __gtdf2 __gedf2 __cmpdf2 __addsf3 __subsf3 __mulsf3 __divsf3 __eqsf2 __nesf2 __ltsf2 __lesf2 __gtsf2 __gesf2 __floatsidf __floatunsidf __floatdidf __floatundidf __floatsisf __floatunsisf __fixdfsi __fixunsdfsi __fixdfdi __fixunsdfdi __fixsfsi __fixunssfsi __fixunssfdi __extendsfdf2 __truncdfsf2"
for s in $syms; do echo "$s::"; done > $T/colon.s
bash z80_lib_symbols.sh $T || { echo "FAIL: name:: definitions not accepted"; exit 1; }
for s in $syms; do printf '\t.globl %s\n' "$s"; done > $T/colon.s
bash z80_lib_symbols.sh $T || { echo "FAIL: .globl definitions not accepted"; exit 1; }
grep -v __truncdfsf2 $T/colon.s > $T/short.s; mv $T/short.s $T/colon.s
out=$(bash z80_lib_symbols.sh $T || true)
[ "$out" = "FAIL: not defined: __truncdfsf2" ] || { echo "FAIL: a missing symbol was not named: $out"; exit 1; }
echo "ok: z80_lib_symbols accepts name:: and .globl, names a missing symbol"
