#!/bin/bash
# tools/cmp_ra.sh SRC.c NAME LEVEL: is the allocator at the pin's on our own IR?
# Compiles SRC at m32 with llrm-cc, takes the last mid-end stage (LLRM_MIR_STAGES), strips what only
# llrm knows, hands that IR to the pin's llc, and prints per function: instructions and stack-slot
# references (esp/ebp operands), ours then the pin's. Static counts, not executed ones.
# Env: LLRM_CC (llrm-cc, default target/release/llrm-cc), PIN_LLC (default ~/scratch/llvm-build/bin/llc),
#      WORK (scratch dir on disk, not tmpfs; default ~/scratch/cmp_ra).
set -eu
src=$1; name=$2; level=$3
cc=${LLRM_CC:-target/release/llrm-cc}; pin=${PIN_LLC:-$HOME/scratch/llvm-build/bin/llc}
work=${WORK:-$HOME/scratch/cmp_ra}/$name-$level; mkdir -p "$work"
LLRM_MIR_STAGES=$work LLRM_CC_TARGET=x86_32-linux "$cc" -O"$level" -mabi=sysv -march=i486 -c "$src" -o "$work/ours.o"
last=$(ls "$work" | grep '\.ll$' | tail -1)
sed -e 's/ llrm-[a-z-]*//g' -e 's/ nearcode//g' -e 's/cc 1003 //g' "$work/$last" > "$work/ir.ll"
sed -i 's/^target datalayout.*/&\ntarget triple = "i386-pc-linux-gnu"/' "$work/ir.ll"
"$pin" -O"$level" -mtriple=i386-pc-linux-gnu -mcpu=i486 -relocation-model=static -filetype=obj "$work/ir.ll" -o "$work/pin.o"
for f in ours pin; do
  objdump -d -M intel --no-show-raw-insn "$work/$f.o" | sed 's/^ *[0-9a-f]*:\t//' |
    awk '/^[0-9a-f]+ <.*>:/{name=$2} name != "" {n[name]++; if ($0 ~ /PTR \[(esp|ebp)/) s[name]++} END{for (k in n) print k, n[k], s[k]+0}' | sort > "$work/$f.sum"
done
join "$work/ours.sum" "$work/pin.sum" | awk '{print $1, "ours", $2, $3, "pin", $4, $5}'
