#!/bin/bash
# usage: libgcc2.sh OUTDIR -> stock libgcc2.c members of GCC 15.2 for m68k (i64 arithmetic and conversions), built with the stock m68k cross gcc, one object per member, <cpu>_<member>.o, cpu 68000 and 68020, -Os.
# Provenance: sources ~/scratch/sf-libgcc-src/libgcc (GCC 15.2 tarball), config/m68k/longlong.h as shipped; the generated tm.h/libgcc_tm.h of a real build are replaced by two defines (word size 4) and -D__LIBGCC_HAS_SF_MODE__/DF_MODE__ (the compiler predefines them in a real build):
# the members come out of the same libgcc2.c code with its m68k longlong.h, not from an installed libgcc.a (the cross toolchain here has none).
set -e
O=${1:?outdir}; mkdir -p $O; S=${LIBGCC_SRC:-$HOME/scratch/sf-libgcc-src/libgcc}
export LD_LIBRARY_PATH=$HOME/scratch/gcc-m68k/root/usr/lib/x86_64-linux-gnu; B=$HOME/scratch/gcc-m68k/root/usr/bin/m68k-linux-gnu
for h in tconfig.h coretypes.h libgcc_tm.h; do : > $O/$h; done
printf 'typedef __SIZE_TYPE__ size_t;\n' > $O/tsystem.h; printf '#define MIN_UNITS_PER_WORD 4\n#define LIBGCC2_UNITS_PER_WORD 4\n' > $O/tm.h
for c in 68000 68020; do for m in muldi3 divdi3 udivdi3 moddi3 umoddi3 udivmoddi4 ashldi3 lshrdi3 ashrdi3 floatdidf floatundidf fixdfdi fixunsdfdi fixunsdfsi clz; do
  $B-gcc -msoft-float -m$c -Os -ffreestanding -DIN_LIBGCC2 -D__LIBGCC_HAS_SF_MODE__=1 -D__LIBGCC_HAS_DF_MODE__=1 -D__LIBGCC_SF_MANT_DIG__=24 -D__LIBGCC_DF_MANT_DIG__=53 -DL_$m -I$O -I$S -I$S/../include -I$S/config/m68k -c $S/libgcc2.c -o $O/${c}_$m.o
done; done
