#!/bin/bash
# Soft-float library bytes per operation, ours (llrm-cc, runtime/compiler-rt/lib/fp) vs compiler-rt (pin builtins, clang) vs libgcc soft-fp (host gcc, m32 only; gcc has no wasm).
# Bytes = code + read-only data of one object per routine, no helpers, no unwind tables.
# usage: rows_bytes.sh m32|wasm32 -> table per -Os and -O2 for f32 and f64.   Env: LLRM_CC_DIR (llrm-cc), CRT (builtins dir), LIBGCC (gcc libgcc dir), CLANG_WASM.
set -u
tgt=${1:?m32 or wasm32}; root=$(cd "$(dirname "$0")/../.." && pwd); rt=$root/runtime/compiler-rt
cc=${LLRM_CC_DIR:?dir with llrm-cc}/llrm-cc; CRT=${CRT:-$HOME/scratch/llvm-src/compiler-rt/lib/builtins}; S=${LIBGCC:-$HOME/scratch/sf-libgcc-src/libgcc}
T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT
case $tgt in
  m32) ccw="x86_32-linux"; cl="/usr/bin/clang --target=i486-linux-gnu -fno-pic -fno-asynchronous-unwind-tables"; extra="-march=i486"
       bytes() { size -A "$1" | awk '$1==".text"||$1~/^\.rodata/{s+=$2} END{print s+0}'; } ;;
  wasm32) ccw="wasm32"; cl="${CLANG_WASM:-$HOME/scratch/llvm-build-clang-wasm/bin/clang} --target=wasm32 -ffreestanding"; extra=""
       bytes() { local s=0 h; for h in $(wasm-objdump -h "$1" | awk '$1=="Code"||$1=="Data"' | grep -o 'size=0x[0-9a-f]*' | cut -d= -f2); do s=$((s + h)); done; echo $s; } ;;
  *) echo "target: m32 or wasm32" >&2; exit 2 ;;
esac
# op: ours file | crt routines (summed) | libgcc routines (summed)
ops="add+sub:add_sub:adddf3,subdf3:addsf3,subsf3 mul:mul:muldf3:mulsf3 div:div:divdf3:divsf3 neg:neg:negdf2:negsf2 fix_si:fix_si:fixdfsi:fixsfsi fix_usi:fix_usi:fixunsdfsi:fixunssfsi float:float:floatsidf,floatunsidf,floatdidf,floatundidf:floatsisf cmp:cmp:comparedf2:comparesf2"
for O in Os O2; do
  echo "== $tgt -$O (bytes)"; printf "%-9s %7s %7s %7s | %7s %7s %7s\n" op ours64 crt64 gcc64 ours32 crt32 gcc32
  for spec in $ops; do IFS=: read name file c64 c32 <<<"$spec"
    row=()
    for w in 64 32; do
      d=f$w; sfx=$([ $w = 64 ] && echo df || echo sf); of=$file
      [ $w = 64 ] && f=$file || f=$file
      src=$rt/lib/fp/$d/$f.c; [ $name = float ] && [ $w = 32 ] && src=$rt/lib/fp/f32/float_si.c
      o=ERR; if LLRM_CC_TARGET=$ccw "$cc" $extra -$O -c -I$rt/include "$src" -o $T/o.o 2>/dev/null; then o=$(bytes $T/o.o); fi
      list=$c64; [ $w = 32 ] && list=$c32
      c=0; g=0
      for r in ${list//,/ }; do
        if $cl $extra -$O -w -I$CRT -c $CRT/$r.c -o $T/c.o 2>/dev/null; then c=$((c + $(bytes $T/c.o))); else c=ERR; break; fi
        if [ $tgt = m32 ]; then
          if gcc -m32 -march=i486 -$O -fno-pic -fno-stack-protector -fcf-protection=none -fno-asynchronous-unwind-tables -w -I$S/soft-fp -I$S/config/i386 -I$S -I$S/../include -c $S/soft-fp/$r.c -o $T/g.o 2>/dev/null; then [ $g != ERR ] && g=$((g + $(bytes $T/g.o))); else g=ERR; fi
        else g=-; fi
      done
      row+=("$o" "$c" "$g")
    done
    printf "%-9s %7s %7s %7s | %7s %7s %7s\n" "$name" "${row[@]}"
  done
done
