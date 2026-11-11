#!/bin/bash
# usage: LLRM_Z80_LLC=<llrm-z80-llc> tests/z80_clz_shared_test.sh -> no soft-float module defines its own _sf_clz32: llrm-z80-llc emits the inline clz as a global (~210 B in each of 7 modules, "Multiple definition" at link); one __clzsi2 is shared.
set -e
cd "$(dirname "$0")/.."; T=$(mktemp -d); trap 'rm -r $T' EXIT
bash tests/z80_lib.sh $T/lib Os > /dev/null
own=$(grep -l '^_sf_clz32::' $T/lib/*.s || true)
[ -z "$own" ] || { echo "FAIL: own copy of _sf_clz32 in: $(echo $own | xargs -n1 basename | tr '\n' ' ')"; exit 1; }
[ "$(grep -l '^__clzsi2::' $T/lib/*.s | wc -l)" = 1 ] || { echo "FAIL: __clzsi2 must be defined by exactly one module"; exit 1; }
echo "ok: no private _sf_clz32; $(awk '/^A _CODE/{print $4; exit}' $T/lib/fp_f64_mul.rel | xargs -I{} sh -c 'echo $((0x{}))') B f64 mul"
