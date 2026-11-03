#!/bin/bash
# usage: LLRM_CC_DIR=dir tests/specials_once_gate.sh -> specials out of the common mul/div path: SF_SPECIALS_ONCE (both) in the m32 header, SF_SPECIALS_MUL in the m16
# header, each at -O2 only: at -Os (+29 B) it must not. Fails if the gate is lost either way.
set -e
cd "$(dirname "$0")/.."; D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; T=$(mktemp -d); trap 'rm -r $T' EXIT
n() { # target header knob opt
  printf '#include "%s"\n#ifdef %s\non\n#endif\n' $2 $3 > $T/p.c; LLRM_CC_TARGET=$1 $D/llrm-cc -march=i486 -$4 -E -Iinclude -I. $T/p.c | grep -c '^on$' || true; }
o2=$(n x86_32-linux config/x86_m32/config.h SF_SPECIALS_ONCE O2); os=$(n x86_32-linux config/x86_m32/config.h SF_SPECIALS_ONCE Os)
m2=$(n x86_16-wat config/x86_m16/config.h SF_SPECIALS_MUL O2); ms=$(n x86_16-wat config/x86_m16/config.h SF_SPECIALS_MUL Os)
[ "$o2" = 1 ] && [ "$os" = 0 ] && [ "$m2" = 1 ] && [ "$ms" = 0 ] || { echo "FAIL: m32 knob -O2 $o2 -Os $os, m16 knob -O2 $m2 -Os $ms (want 1 0 1 0)"; exit 1; }
echo "ok: knobs on at -O2, off at -Os (m32, m16)"
