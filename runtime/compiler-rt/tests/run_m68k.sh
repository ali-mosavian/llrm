#!/bin/sh
# usage: LLRM_CC_DIR=dir tests/run_m68k.sh CPU OPT [n]   - vectors from the host FPU, library built by llrm-cc, run under qemu-m68k.
set -e
cd "$(dirname "$0")/.."
CPU=$1; OPT=$2; N=${3:-3000}
T=$(mktemp -d)
cc -O2 -ffp-contract=off tests/gen_vectors.c -o $T/gen && $T/gen $N > $T/vectors.h
CFLAGS_EXTRA="-I$PWD/include -I$T" ${M68K_RUN_C:-$HOME/scratch/handoff/m68k-run-c.sh} $CPU $OPT tests/m68k_main.c lib/fp/sf32.c lib/fp/sf32_conv.c lib/int/int32.c
md5sum $T/vectors.h | cut -c1-12
