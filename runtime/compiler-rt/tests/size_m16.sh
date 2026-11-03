#!/bin/bash
# usage: tests/size_m16.sh [OPT] -> code bytes at x86_16-wat (llrm-cc, -march=i486): ours (sf32.c + sf32_conv.c) and compiler-rt (the same symbols). Library bytes only.
# Needs LLRM_CLANG_M16 (see crates/target/llrm-x86-m16), LLRM_OW; LLRM_CC_DIR holds llrm-cc and llrm-llc.
set -e
cd "$(dirname "$0")/.."
OPT=${1:-Os}; D=${LLRM_CC_DIR:-$HOME/targets/gs-out/rbase}; CRT=${COMPILER_RT:-$HOME/scratch/llvm-src-m16/compiler-rt/lib/builtins}
T=$(mktemp -d); trap 'rm -r $T' EXIT; export LLRM_CC_TARGET=x86_16-wat
cc1() { $D/llrm-cc -march=i486 -$OPT -c "$@" 2>$T/err || { echo "  compile failed: $(head -c 100 $T/err)"; return 1; }; }
sz() { python3 tests/omf_code_bytes.py "$1"; }
echo "== x86_16-wat -march=i486 -$OPT, code bytes"
tot=0; for f in sf32 sf32_conv; do cc1 -Iinclude $(ls lib/*/$f.c) -o $T/$f.obj && s=$(sz $T/$f.obj) && echo "ours $f $s" && tot=$((tot+s)); done; echo "ours total $tot"
tot=0; for n in addsf3 subsf3 mulsf3 divsf3 negsf2 fixsfsi fixunssfsi floatsisf floatunsisf comparesf2; do
  if cc1 -I$CRT $CRT/$n.c -o $T/c_$n.obj; then s=$(sz $T/c_$n.obj); echo "compiler-rt $n $s"; tot=$((tot+s)); fi; done; echo "compiler-rt total $tot"
