#!/bin/bash
# usage: LLRM_CC_DIR=dir tests/specials_once_gate.sh -> SF_SPECIALS_ONCE (specials out of the common mul/div path) is on in the m32 header at -O2 only: at -Os (+29 B) it must not. Fails if the gate is lost either way.
set -e
cd "$(dirname "$0")/.."; D=${LLRM_CC_DIR:?set LLRM_CC_DIR}; T=$(mktemp -d); trap 'rm -r $T' EXIT
n() { echo '#include "sf_x86_m32.h"
#ifdef SF_SPECIALS_ONCE
on
#endif' > $T/p.c; LLRM_CC_TARGET=x86_32-linux $D/llrm-cc -march=i486 -$1 -E -Iinclude $T/p.c | grep -c '^on$' || true; }
o2=$(n O2); os=$(n Os)
[ "$o2" = 1 ] && [ "$os" = 0 ] || { echo "FAIL: knob at -O2 $o2 (want 1), at -Os $os (want 0)"; exit 1; }
echo "ok: SF_SPECIALS_ONCE on at -O2, off at -Os"
