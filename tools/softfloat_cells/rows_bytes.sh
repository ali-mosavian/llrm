#!/bin/bash
# Soft-float library bytes per operation, ours (llrm-cc, runtime/compiler-rt/lib/fp) vs compiler-rt (pin builtins, clang) vs libgcc soft-fp (host gcc, m32 only; gcc has no wasm).
# Ours is built with its target config (config/x86_m32 or config/wasm, as the library tests do); compiler-rt and libgcc as shipped.
# Bytes = code + read-only data of one object per routine, no helpers, no unwind tables.
# m32: compiler-rt (clang) returns float/double in st0 (add+sub mul div neg float): its bytes carry the x87 return (neg is `flds; fchs`, 7 B, against the integer-register
# convention ours uses), so those cells print with a `*` and are not a rank; libgcc is built -msoft-float (integer-register return, as ours).
# usage: rows_bytes.sh m32|wasm32 -> table per -Os and -O2 for f32 and f64.   Env: ROWS_ONLY (one op name), LLRM_CC_DIR (llrm-cc), CRT (builtins dir), LIBGCC (gcc libgcc dir), CLANG_WASM.
set -u
tgt=${1:?m32 or wasm32}; root=$(cd "$(dirname "$0")/../.." && pwd); rt=$root/runtime/compiler-rt
cc=${LLRM_CC_DIR:?dir with llrm-cc}/llrm-cc; CRT=${CRT:-$HOME/scratch/llvm-src/compiler-rt/lib/builtins}; S=${LIBGCC:-$HOME/scratch/sf-libgcc-src/libgcc}
T=$(mktemp -d); trap 'rm -rf "${T:?}"' EXIT
case $tgt in
  m32) ccw="x86_32-linux"; cl="/usr/bin/clang --target=i486-linux-gnu -fno-pic -fno-asynchronous-unwind-tables"; extra="-march=i486"; cfg="-DSF_INTABI=1 -include stdint.h -include $rt/config/x86_m32/config.h"
       bytes() { size -A "$1" | awk '$1~/^\.text/||$1~/^\.rodata/{s+=$2} END{print s+0}'; } ;;
  wasm32) ccw="wasm32"; cl="${CLANG_WASM:-$HOME/scratch/llvm-build-clang-wasm/bin/clang} --target=wasm32 -ffreestanding"; extra=""; cfg="-include stdint.h -include $rt/config/wasm/config.h"
       bytes() { local s=0 h; for h in $(wasm-objdump -h "$1" | awk '$1=="Code"||$1=="Data"' | grep -o 'size=0x[0-9a-f]*' | cut -d= -f2); do s=$((s + h)); done; echo $s; } ;;
  *) echo "target: m32 or wasm32" >&2; exit 2 ;;
esac
# The x87 return shim: what moving an integer-register result into st0 costs (clang -O2 i486: a wrapper of g() returning float/double, less the tail-call wrapper), added
# to ours in the `+x87` columns so a starred compiler-rt cell can be ranked against a like-for-like one.
shim64=0; shim32=0
if [ $tgt = m32 ]; then
  printf 'extern unsigned long long g64(void); extern unsigned g32(void);\ndouble w64(void) { union { unsigned long long u; double d; } x; x.u = g64(); return x.d; }\nfloat w32(void) { union { unsigned u; float f; } x; x.u = g32(); return x.f; }\nunsigned long long b64(void) { return g64(); }\nunsigned b32(void) { return g32(); }\n' > $T/shim.c
  /usr/bin/clang --target=i486-linux-gnu -march=i486 -O2 -fno-pic -fno-asynchronous-unwind-tables -c $T/shim.c -o $T/shim.o
  sz() { echo $((16#$(nm -S $T/shim.o | awk -v n=$1 '$4==n{print $2}'))); }
  shim64=$(( $(sz w64) - $(sz b64) )); shim32=$(( $(sz w32) - $(sz b32) ))
fi
# op: ours file | crt routines (summed) | libgcc routines (summed)
ops="add+sub:add_sub:adddf3,subdf3:addsf3,subsf3 mul:mul:muldf3:mulsf3 div:div:divdf3:divsf3 neg:neg:negdf2:negsf2 fix_si:fix_si:fixdfsi:fixsfsi fix_usi:fix_usi:fixunsdfsi:fixunssfsi float:float:floatsidf,floatunsidf,floatdidf,floatundidf:floatsisf cmp:cmp:comparedf2:comparesf2:eqdf2,ledf2,gedf2,unorddf2:eqsf2,lesf2,gesf2,unordsf2"
for O in Os O2; do
  echo "== $tgt -$O (bytes)"; printf "%-9s %7s %7s %7s | %7s %7s %7s | %9s %9s\n" op ours64 crt64 gcc64 ours32 crt32 gcc32 ours64+x87 ours32+x87
  for spec in $ops; do IFS=: read name file c64 c32 g64 g32 <<<"$spec"
    [ -z "${ROWS_ONLY:-}" ] || [ "$name" = "$ROWS_ONLY" ] || continue
    row=()
    for w in 64 32; do
      d=f$w; sfx=$([ $w = 64 ] && echo df || echo sf); of=$file
      [ $w = 64 ] && f=$file || f=$file
      src=$rt/lib/fp/$d/$f.c; [ $name = float ] && [ $w = 32 ] && src=$rt/lib/fp/f32/float_si.c
      o=ERR; if LLRM_CC_TARGET=$ccw "$cc" $extra $cfg -$O -c -I$rt/include "$src" -o $T/o.o 2>/dev/null; then o=$(bytes $T/o.o); fi
      # compiler-rt's comparesf2/comparedf2 object also holds __unordsf2/__unorddf2: ours is cmp.c plus unord.c
      if [ $name = cmp ] && [ "$o" != ERR ]; then
        u=ERR; if LLRM_CC_TARGET=$ccw "$cc" $extra $cfg -$O -c -I$rt/include "$rt/lib/fp/$d/unord.c" -o $T/u.o 2>/dev/null; then u=$(bytes $T/u.o); fi
        [ "$u" = ERR ] && o=ERR || o=$((o + u))
      fi
      list=$c64; [ $w = 32 ] && list=$c32
      glist=${g64:-$c64}; [ $w = 32 ] && glist=${g32:-$c32}
      c=0; g=0
      for r in ${list//,/ }; do
        if $cl $extra -$O -w -I$CRT -c $CRT/$r.c -o $T/c.o 2>/dev/null; then c=$((c + $(bytes $T/c.o))); else c=ERR; break; fi
      done
      # libgcc has no comparedf2: its compare set is eq, le, ge (ne, lt, gt reuse them) and unord, so cmp takes its own list (glist)
      if [ $tgt = m32 ]; then
        for r in ${glist//,/ }; do
          if gcc -m32 -march=i486 -$O -falign-functions=1 -falign-jumps=1 -falign-loops=1 -falign-labels=1 -fno-pic -fno-stack-protector -fcf-protection=none -fno-asynchronous-unwind-tables -msoft-float -w -I$S/soft-fp -I$S/config/i386 -I$S -I$S/../include -c $S/soft-fp/$r.c -o $T/g.o 2>/dev/null; then [ $g != ERR ] && g=$((g + $(bytes $T/g.o))); else g=ERR; fi
        done
      else g=-; fi
      # m32 compiler-rt int->float is x87 arithmetic (fild/fmul/fadd), not a soft-float routine: no rank
      [ $tgt = m32 ] && [ $name = float ] && c=fpu
      # wasm32 compiler-rt floatdidf/floatundidf use f64.convert, f64.mul and f64.add (the machine has an FPU): the f64 cell is not a soft-float rank
      [ $tgt = wasm32 ] && [ $name = float ] && [ $w = 64 ] && c=fpu
      case $name in add+sub|mul|div|neg|float) [ $tgt = m32 ] && [ "$c" != fpu ] && [ "$c" != ERR ] && c="$c*";; esac
      row+=("$o" "$c" "$g")
    done
    x64=-; x32=-
    case "${row[1]}" in *"*") [ "${row[0]}" = ERR ] || x64=$((${row[0]} + shim64));; esac
    case "${row[4]}" in *"*") [ "${row[3]}" = ERR ] || x32=$((${row[3]} + shim32));; esac
    printf "%-9s %7s %7s %7s | %7s %7s %7s | %9s %9s\n" "$name" "${row[@]}" $x64 $x32
  done
done
