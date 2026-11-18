#!/bin/bash
# Regression: the flattened m16 soft-double bench kept SF_CMP_SPLIT from the config, so cmp_lt.c .. cmp_cmp.c each pulled cmp.c in again
# and the bench failed to compile (redefinition of sf_cmp_un). The library must be generated with SF_CMP_SPLIT undefined.
cd "$(dirname "$0")/.." || exit 1
grep -q "#undef SF_CMP_SPLIT" tests/x86_m16_soft_double.sh || { echo "x86_m16_soft_double.sh does not undef SF_CMP_SPLIT"; exit 1; }
