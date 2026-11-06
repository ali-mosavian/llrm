#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc tests/asm6502/fix_si_cycles_test.sh
# The one-shift fix_si.c cost 6502 __fixsfsi 44352 net cycles at -Os (+15% over the 38535 of the two-direction form): the shift libcall loops per bit.
# SF_SHIFT_LOOP (config/6502) keeps the two-direction form. Bound 38600.
net=$(sh "$(dirname "$0")/bench_fix.sh" Os | sed -n 's/^__fixsfsi.*(net on \([0-9]*\),.*/\1/p')
[ -n "$net" ] && [ "$net" -le 38600 ] && echo "fix_si_cycles ok ($net)" || { echo "FAIL: __fixsfsi -Os net cycles '$net' (bound 38600)"; exit 1; }
