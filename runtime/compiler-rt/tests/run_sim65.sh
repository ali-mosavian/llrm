#!/bin/sh
# usage: LLRM_CC=path/to/llrm-cc [MOS_RT=mos_rt.s] tests/run_sim65.sh [n]
# SF6502 holds the 6502 configuration flags of config/6502/config.h (default -include config/6502/config.h); SF6502= builds the library defaults.
# One ca65 file (equates + _main wrapper + compiled library + mos_rt.s), cl65 -t sim6502, sim65: the 6502 harness shape.
set -e
cd "$(dirname "$0")/.."
N=${1:-150}; T=$(mktemp -d); OPT=${OPT:-Os}
MOS_RT=${MOS_RT:-$HOME/scratch/handoff/mos_rt.s}
cc -O2 -ffp-contract=off tests/gen_vectors.c -o $T/gen && $T/gen $N > $T/vectors.h
X=; [ -n "${SF6502--x}" ] && X=sf32_6502 # the 6502 config takes mul/div from config/6502/sf32_6502.c
for f in tests/sim65_main.c $(tests/fp_sources.sh f32) ${X:+config/6502/$X.c}; do
  $LLRM_CC --target=6502-sim65 -$OPT -S -DSF_INTABI ${SF6502--include $PWD/config/6502/config.h} -Iinclude -I$T $f -o $T/$(basename $f .c).s
done
{ grep -h '^rc[0-9]* = ' $T/sim65_main.s | sort -u
  printf '.export _main\n_main:\n  jsr main\n  rts\n'
  for f in sim65_main $(for s in $(tests/fp_sources.sh f32); do basename $s .c; done) $X; do grep -v '^rc[0-9]* = ' $T/$f.s || true; done
  cat $MOS_RT; } > $T/p.s
cl65 -t sim6502 -o $T/p.bin $T/p.s
sim65 -c -x 500000000 $T/p.bin; echo "exit=$?"; md5sum $T/p.bin | cut -c1-12; md5sum $T/vectors.h | cut -c1-12
