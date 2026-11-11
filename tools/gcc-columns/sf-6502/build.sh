#!/bin/bash
# usage: build.sh CC1 DIR LEVEL   (LEVEL = O2 | Os): compile compiler-rt's f32 sources with the 6502 cc1, assemble them with ca65 and
# archive them: DIR/generic.lib (generic C) and DIR/x6502.lib (the 6502 config's knobs, mul/div from config/6502/sf32_6502.c).
set -e
here=$(cd "$(dirname "$0")" && pwd); S=$here/../../../runtime/compiler-rt
CC1=$1; O=$2; L=$3; mkdir -p "$O/g" "$O/x"; ulimit -v 16000000
F="add_sub mul div pack float_si float_usi fix_si fix_usi"
cc() { # SRC OUT [extra flags]
  local src=$1 out=$2; shift 2
  timeout 300 "$CC1" -quiet -"$L" -I"$here/../sf-z80/inc" -I"$S/include" -DSF_INTABI "$@" "$src" -o "$out.s" || { echo "cc1 failed $src $L"; exit 1; }
  ca65 -t sim6502 -o "$out.o" "$out.s"
}
for f in $F; do cc "$S/lib/fp/f32/$f.c" "$O/g/f32_$f"; done
cc "$S/lib/int/int32.c" "$O/g/int32"
for f in $F; do [ "$f" = mul ] || [ "$f" = div ] || cc "$S/lib/fp/f32/$f.c" "$O/x/f32_$f" -include "$S/config/6502/config.h"; done
cc "$S/config/6502/sf32_6502.c" "$O/x/sf32_6502" -include "$S/config/6502/config.h"
cp "$O/g/int32.o" "$O/x/int32.o"
for h in "$here"/../rt-6502/*.s "$here"/rt/*.s; do b=$(basename "$h" .s); ca65 -t sim6502 -o "$O/g/rt_$b.o" "$h"; cp "$O/g/rt_$b.o" "$O/x/"; done
rm -f "$O/generic.lib" "$O/x6502.lib"; ar65 a "$O/generic.lib" "$O"/g/*.o; ar65 a "$O/x6502.lib" "$O"/x/*.o
