#!/bin/bash
# build.sh PROG: llrm (OMF; ELF as llrmElfO2/llrmElfOs), gcc and clang (ELF) at -O1, -O2, -O3 and -Os into $VSGCC_WORK, flags as tools/bench plus -fno-inline-functions
set -e
P=$1; R=$(cd "$(dirname "$0")/../../../.." && pwd); O=${VSGCC_WORK:-$HOME/scratch/vsgcc-work}; K=$(cd "$(dirname "$0")" && pwd)/kernels; if [ -d $K/$P ]; then S=$K/$P/$P.c; else S=$R/bench/$P/$P.c; fi
LLRM=${LLRM:-$(python3 "$R/tools/llrmbin.py" bin)/llrm-c}
mkdir -p $O/b $O/o
# llrm side in the convention gcc and clang use, or the table measures register arguments against stack ones
ABI=${VSGCC_ABI:-sysv}
echo -n $ABI > $O/abi
EMU=$(python3 "$R/tools/linkrecipe.py" x86-m32 ld-emulation)
$LLRM -m32 -mabi=$ABI -O2 -march=i486 -fno-inline-functions -o $O/o/$P.llrm.obj $S 2>$O/o/$P.llrm.err || echo "FAIL llrm $P"
$LLRM -m32 -mabi=$ABI -Os -march=i486 -fno-inline-functions -o $O/o/$P.llrmOs.obj $S 2>$O/o/$P.llrmOs.err || echo "FAIL llrmOs $P"
for o in O1 O3; do $LLRM -m32 -mabi=$ABI -$o -march=i486 -fno-inline-functions -o $O/o/$P.llrm$o.obj $S 2>$O/o/$P.llrm$o.err || echo "FAIL llrm$o $P"; done
cd $O/b
ORIG=$S
# the kernel must stay a call: gcc/clang inline it into main otherwise (llrm does not under -fno-inline-functions)
sed -E "s/^([a-z][a-z ]*[ *])(bench_$P\()/__attribute__((noinline)) \1\2/" $S > $P.c
grep -q noinline $P.c || echo "NO NOINLINE $P"
S=$O/b/$P.c
F="-Dfar= -m32 -march=i486 -fno-pic -fno-inline-functions -fno-stack-protector -fcf-protection=none -fno-asynchronous-unwind-tables"
for o in O1 O2 O3 Os; do
  gcc $F -$o -c -o $P.gcc$o.o $S 2>$P.gcc$o.err || echo "FAIL gcc$o $P"
  clang $F -$o -c -o $P.clang$o.o $S 2>$P.clang$o.err || echo "FAIL clang$o $P"
  CS="gcc clang"
  if [ $o = O2 ] || [ $o = Os ]; then  # the ELF build of llrm is for loops.py
    $LLRM -m32 -mabi=$ABI -$o -march=i486 -fno-inline-functions -fobject-format=elf -o $P.llrmElf$o.o $ORIG 2>$P.llrmElf$o.err || echo "FAIL llrmElf$o $P"
    CS="gcc clang llrmElf"
  fi
  for c in $CS; do
    ld -m $EMU -static -e 0 -Ttext=0x10000 --just-symbols=$O/stub.elf -o $P.$c$o.elf $P.$c$o.o 2>$P.$c$o.lderr || echo "LINK FAIL $c$o $P: $(grep -o 'undefined reference to.*' $P.$c$o.lderr | sort -u | tr '\n' ' ')"
  done
done
