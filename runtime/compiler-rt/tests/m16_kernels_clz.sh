#!/bin/sh
# The m16 -Os config calls __clzsi2 (SF_CLZ_CALL): the flattened one-file benches must define it, or every -Os link fails with an undefined ___clzsi2@3
# (jwlink E2028 on sfm16_{add,mul,div}, found when the clock cells were rerun).
cd "$(dirname "$0")/.." || exit 1; T=$(mktemp -d); trap 'rm -r $T' EXIT
LLRM_CLANG_M16=${LLRM_CLANG_M16:-cc} bash tests/x86_m16_kernels.sh 4 $T > /dev/null 2>&1
for n in sfm16_add sfm16_mul sfm16_div; do
  grep -q 'int SFB_NAME(clzsi2)(sfu32 x)' $T/$n/$n.c || { echo "FAIL: $n has no __clzsi2 definition"; exit 1; }
done
echo "ok: the flattened m16 kernels define __clzsi2"
