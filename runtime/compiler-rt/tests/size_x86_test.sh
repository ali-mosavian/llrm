#!/bin/sh
# size_x86.sh built "ours" without config/x86_m32 (no widening multiply): f64 mul read 1962 B and 2x compiler-rt's clocks against 1681 B and +17% as shipped.
cd "$(dirname "$0")"
grep -q 'ours() {.*-include config/x86_m32/config.h' size_x86.sh || { echo "FAIL: ours() lacks the x86_m32 config"; exit 1; }
grep -q 'ours -Iinclude' size_x86.sh || { echo "FAIL: ours columns bypass ours()"; exit 1; }
echo "size_x86 ok"
