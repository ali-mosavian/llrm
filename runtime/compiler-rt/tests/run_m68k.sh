#!/bin/sh
# M68K_FLOAT_C=1 calls plain float C (tests/m68k_float_ops.c, SoftenFloat) instead of the library's canonical names.
# usage: LLRM_CC_DIR=dir tests/run_m68k.sh CPU OPT [n]   - vectors from the host FPU, library built by llrm-cc, run under qemu-m68k.
set -e
cd "$(dirname "$0")/.."
CPU=$1; OPT=$2; N=${3:-3000}
T=$(mktemp -d)
cc -O2 -ffp-contract=off tests/gen_vectors.c -o $T/gen && $T/gen $N > $T/vectors.h
CFLAGS_EXTRA="-I$PWD/include -I$T ${M68K_FLOAT_C:+-DM68K_FLOAT_C}" ${M68K_RUN_C:-$HOME/scratch/handoff/m68k-run-c.sh} $CPU $OPT tests/m68k_main.c $(tests/fp_sources.sh f32) lib/int/int32.c ${M68K_FLOAT_C:+tests/m68k_float_ops.c}
md5sum $T/vectors.h | cut -c1-12
