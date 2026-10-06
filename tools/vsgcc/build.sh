#!/bin/bash
# build.sh PROG: llrm (OMF), gcc and clang (ELF) at -O2 and -Os into $VSGCC_WORK, flags as tools/bench plus -fno-inline-functions
set -e
P=$1; R=$(cd "$(dirname "$0")/../.." && pwd); O=${VSGCC_WORK:-$HOME/scratch/vsgcc-work}; S=$R/bench/$P/$P.c
LLRM=${LLRM:-$R/target/release/llrm-c}
mkdir -p $O/b $O/o
$LLRM --target x86-code32 -O2 -march=i486 -fno-inline-functions -o $O/o/$P.llrm.obj $S 2>$O/o/$P.llrm.err || echo "FAIL llrm $P"
$LLRM --target x86-code32 -Os -march=i486 -fno-inline-functions -o $O/o/$P.llrmOs.obj $S 2>$O/o/$P.llrmOs.err || echo "FAIL llrmOs $P"
cd $O/b
# the kernel must stay a call: gcc/clang inline it into main otherwise (llrm does not under -fno-inline-functions)
sed -E "s/^([a-z][a-z ]*[ *])(bench_$P\()/__attribute__((noinline)) \1\2/" $S > $P.c
grep -q noinline $P.c || echo "NO NOINLINE $P"
S=$O/b/$P.c
F="-Dfar= -m32 -march=i486 -fno-pic -fno-inline-functions -fno-stack-protector -fcf-protection=none -fno-asynchronous-unwind-tables"
for o in O2 Os; do
  gcc $F -$o -c -o $P.gcc$o.o $S 2>$P.gcc$o.err || echo "FAIL gcc$o $P"
  clang $F -$o -c -o $P.clang$o.o $S 2>$P.clang$o.err || echo "FAIL clang$o $P"
  for c in gcc clang; do
    ld -m elf_i386 -static -e 0 -Ttext=0x10000 --just-symbols=$O/stub.elf -o $P.$c$o.elf $P.$c$o.o 2>$P.$c$o.lderr || echo "LINK FAIL $c$o $P: $(grep -o 'undefined reference to.*' $P.$c$o.lderr | sort -u | tr '\n' ' ')"
  done
done
