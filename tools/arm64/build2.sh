#!/bin/bash
ulimit -v 16000000  # one unbounded run must not take the box
# build.sh PROG LEVEL COMP  -> out/PROG.LEVEL.COMP.elf  (+ .o)   COMP: clang|gcc|hand|dagg
H=$(cd "$(dirname "$0")" && pwd)  # this directory: prep.py, host_shim.h, rt.c
P=$1; L=$2; C=$3;  B=${B:-$(cd "$H/../.." && pwd)}; V=${LLVM_BIN:-$HOME/scratch/llvm-build-aarch64/bin}  # B: the tree, V: clang/ld.lld of the pin
O=${OUT:-${TMPDIR:-/tmp}/arm64-bench-out}; mkdir -p $O; mkdir -p $O/src; python3 $H/prep.py $B/bench/$P/$P.c $O/src/$P.c.$$ && mv -f $O/src/$P.c.$$ $O/src/$P.c; S=$O/src/$P.c; X=$O/$P.$L.$C
[ -f $O/bench_io.h ] || cp $B/bench/bench_io.h $O/  # the sources include it from one directory up
LLC="timeout 600 ${LLC:-$B/target/debug/llrm-arm64-llc}"
T="-target aarch64-unknown-linux-gnu -I $B/runtime/shared/posix -Dfar= -Dnear= -Dhuge= -D__huge= -D__386__=1 -include $H/host_shim.h"
case $C in
 cc2) LLRM_ARM64_SYSROOT=/usr/aarch64-linux-gnu LLRM_CC_TARGET=arm64-linux timeout 600 ${CC} -target arm64-linux -O$L -ffreestanding -c $S -o $X.o ;;
 cc4) LLRM_ARM64_SYSROOT=/usr/aarch64-linux-gnu LLRM_CC_TARGET=arm64-linux timeout 600 ${CC} -target arm64-linux -fomit-frame-pointer -O$L -ffreestanding -c $S -o $X.o ;;
 cc5) LLRM_ARM64_SYSROOT=/usr/aarch64-linux-gnu timeout 600 ${CC} $T -fomit-frame-pointer -O$L -ffreestanding -c $S -o $X.o ;;
 clang) $V/clang $T -fno-pic -fno-pie -O$L -ffreestanding -c $S -o $X.o ;;
 clangnv) $V/clang $T -fno-pic -fno-pie -O$L -fno-vectorize -fno-slp-vectorize -ffreestanding -c $S -o $X.o ;;
 gcc) aarch64-linux-gnu-gcc -fno-pic -fno-pie -I $B/runtime/shared/posix -Dfar= -Dnear= -Dhuge= -D__huge= -D__386__=1 -include $H/host_shim.h -O$L -ffreestanding -fno-stack-protector -c $S -o $X.o ;;
 gccnv) aarch64-linux-gnu-gcc -fno-pic -fno-pie -I $B/runtime/shared/posix -Dfar= -Dnear= -Dhuge= -D__huge= -D__386__=1 -include $H/host_shim.h -O$L -fno-tree-vectorize -ffreestanding -fno-stack-protector -c $S -o $X.o ;;
 gccx) aarch64-linux-gnu-gcc $(python3 $H/../external_flags.py --target arm64 --reloc=-fPIE --gcc-major 15) -I $B/runtime/shared/posix -Dfar= -Dnear= -Dhuge= -D__huge= -D__386__=1 -include $H/host_shim.h -O$L -fno-tree-vectorize -ffreestanding -c $S -o $X.o ;;
 clangx) $V/clang $T $(python3 $H/../external_flags.py --target arm64 --reloc=-fPIE) -O$L -fno-vectorize -fno-slp-vectorize -ffreestanding -c $S -o $X.o ;;
 ccomit) LLRM_ARM64_SYSROOT=/usr/aarch64-linux-gnu timeout 600 ${CC:-$B/target/debug/llrm-cc} $T -fomit-frame-pointer -O$L -ffreestanding -c $S -o $X.o ;;
 clangomit) $V/clang $T -fno-pic -fno-pie -fomit-frame-pointer -O$L -ffreestanding -c $S -o $X.o ;;
 cc) LLRM_ARM64_SYSROOT=/usr/aarch64-linux-gnu timeout 600 ${CC:-$B/target/debug/llrm-cc} $T -O$L -ffreestanding -c $S -o $X.o ;;
 pin)
  $V/clang $T -fno-pic -fno-pie -O$L -fno-vectorize -fno-slp-vectorize -ffreestanding -S -emit-llvm -Xclang -emit-llvm-uselists $S -o $X.ll || exit 1
  ${PIN_LLC:-$HOME/scratch/llvm-build-assert-a64/bin/llc} -mtriple=aarch64-unknown-linux-gnu -relocation-model=static -O$([ $L = s ] && echo 2 || echo $L) -filetype=obj -o $X.o $X.ll ;;
 hand|handg|dagg|lin|oldhand)
  $V/clang $T -O$L -fno-vectorize -fno-slp-vectorize -ffreestanding -S -emit-llvm $S -o $X.ll || exit 1
  if [ $C = hand ]; then $LLC -O$L -o $X.o $X.ll; elif [ $C = lin ]; then LLRM_ARM64_LINEAR=1 $LLC -O$L -o $X.o $X.ll; elif [ $C = oldhand ]; then LLRM_ARM64_LINEAR=1 $LLC --isel=hand -O$L -o $X.o $X.ll; elif [ $C = handg ]; then LLRM_ARM64_GREEDY=1 $LLC -O$L -o $X.o $X.ll; else LLRM_ARM64_GREEDY=1 $LLC --isel=dag $([ -n "$NOOPT" ] && echo --no-opt || echo -O$L) -o $X.o $X.ll; fi ;;
esac || exit 1
if [ ! -f $O/rt.o ]; then $V/clang $T -O2 -ffreestanding -c $H/rt.c -o $O/rt.o.$$ && mv -n $O/rt.o.$$ $O/rt.o; rm -f $O/rt.o.$$; fi
$V/ld.lld -static -e main $X.o $O/rt.o -o $X.elf
