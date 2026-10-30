#!/bin/bash
# build.sh PROG: llrm (OMF; ELF as llrmElfO2/llrmElfOs), gcc and clang (ELF) at -O1, -O2, -O3 and -Os into $VSGCC_WORK, flags as tools/bench, without: wrap.py keeps the kernel a call
set -e
P=$1; R=$(cd "$(dirname "$0")/../../../.." && pwd); O=${VSGCC_WORK:-$HOME/scratch/vsgcc-work}; K=$(cd "$(dirname "$0")" && pwd)/kernels; if [ -d $K/$P ]; then S=$K/$P/$P.c; else S=$R/bench/$P/$P.c; fi
LLRM=${LLRM:-$(python3 "$R/tools/llrmbin.py" bin)/llrm-cc}
# llrm-cc takes its target as a row; the Open Watcom tree and the pin come from the environment
export LLRM_OW=${LLRM_OW:-$HOME/scratch/ow-src}
mkdir -p $O/b $O/o
ORIG=$S
# the kernel is called through a volatile pointer in every compiler's copy: it stays a call, and the rest inlines as it would
python3 "$(dirname "$0")/wrap.py" $P $ORIG > $O/b/$P.c
S=$O/b/$P.c
# llrm side in the convention gcc and clang use, or the table measures register arguments against stack ones
ABI=${VSGCC_ABI:-sysv}
echo -n $ABI > $O/abi
EMU=$(python3 "$R/tools/linkrecipe.py" x86-m32 ld-emulation)
# the 64-bit divides gcc and clang call, from each compiler's own 32-bit runtime
# VSGCC_ONLY=llrm: llrm's objects alone, for what compares llrm with itself (m16.py), with no gcc or clang installed
if [ -z "${VSGCC_ONLY:-}" ]; then
RT_gcc=$(gcc -m32 -print-libgcc-file-name)
# the pin's clang only: PATH's is the distro's 20.1.8 (its numbers were read as the pin's); pin_clang.py refuses anything else
CLANG=$(python3 "$R/tools/pin_clang.py")
export LLRM_PIN_CLANG=$CLANG
RT_clang=$($CLANG -m32 --rtlib=compiler-rt -print-libgcc-file-name)
# the pin has no 32-bit compiler-rt on this host: the 64-bit divides come from the host's libgcc (the kernels' code is clang's)
[ -f "$RT_clang" ] || RT_clang=$RT_gcc
fi
$LLRM --target=x86_32-wat ${LLRM_FLAGS:-} -mabi=$ABI -O2 -march=i486 -c -o $O/o/$P.llrm.obj $S 2>$O/o/$P.llrm.err || echo "FAIL llrm $P"
$LLRM --target=x86_32-wat ${LLRM_FLAGS:-} -mabi=$ABI -Os -march=i486 -c -o $O/o/$P.llrmOs.obj $S 2>$O/o/$P.llrmOs.err || echo "FAIL llrmOs $P"
for o in O1 O3; do $LLRM --target=x86_32-wat ${LLRM_FLAGS:-} -mabi=$ABI -$o -march=i486 -c -o $O/o/$P.llrm$o.obj $S 2>$O/o/$P.llrm$o.err || echo "FAIL llrm$o $P"; done
[ -z "${VSGCC_ONLY:-}" ] || exit 0
cd $O/b
# Our m32 target states a stack alignment of 4 (S32); the pin's and gcc's Linux default is 16, which keeps a reserved call frame where 4 pushes.
F="-Dfar= -m32 -march=i486 -fno-pic -fno-stack-protector -fcf-protection=none -fno-asynchronous-unwind-tables"
for o in O1 O2 O3 Os; do
  gcc $F -mpreferred-stack-boundary=2 -$o -c -o $P.gcc$o.o $S 2>$P.gcc$o.err || echo "FAIL gcc$o $P"
  $CLANG $F -mstack-alignment=4 -$o -c -o $P.clang$o.o $S 2>$P.clang$o.err || echo "FAIL clang$o $P"
  CS="gcc clang"
  if [ $o = O2 ] || [ $o = Os ]; then  # the ELF build of llrm is for loops.py
    $LLRM --target=x86_32-wat ${LLRM_FLAGS:-} -mabi=$ABI -$o -march=i486 -fobject-format=elf -c -o $P.llrmElf$o.o $S 2>$P.llrmElf$o.err || echo "FAIL llrmElf$o $P"
    CS="gcc clang llrmElf"
  fi
  for c in $CS; do
    ld -m $EMU -static -e 0 -Ttext=0x10000 --just-symbols=$O/stub.elf -Map=$P.$c$o.map -o $P.$c$o.elf $P.$c$o.o $(eval echo \${RT_$c:-}) 2>$P.$c$o.lderr || echo "LINK FAIL $c$o $P: $(grep -o 'undefined reference to.*' $P.$c$o.lderr | sort -u | tr '\n' ' ')"
  done
done
