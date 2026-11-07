#!/bin/bash
# usage: z80_lib_symbols.sh DIR -> exit 1 naming each soft-float symbol no .s in DIR defines.
# A global is `name::` (what llrm-z80-llc prints) or `.globl name` (the hand asm).
DIR=${1:?library directory}
defined=$( { grep -h '\.globl' "$DIR"/*.s | awk '{print $2}'; grep -hoE '^[A-Za-z_][A-Za-z0-9_]*::' "$DIR"/*.s | sed 's/::$//'; } | sort -u)
missing=""
for s in __adddf3 __subdf3 __muldf3 __divdf3 __eqdf2 __nedf2 __ltdf2 __ledf2 __gtdf2 __gedf2 __cmpdf2 __addsf3 __subsf3 __mulsf3 __divsf3 __eqsf2 __nesf2 __ltsf2 __lesf2 \
         __gtsf2 __gesf2 __floatsidf __floatunsidf __floatdidf __floatundidf __floatsisf __floatunsisf __fixdfsi __fixunsdfsi __fixdfdi __fixunsdfdi \
         __fixsfsi __fixunssfsi __fixunssfdi __extendsfdf2 __truncdfsf2; do
  echo "$defined" | grep -qx "$s" || missing="$missing $s"
done
[ -z "$missing" ] || { echo "FAIL: not defined:$missing"; exit 1; }
