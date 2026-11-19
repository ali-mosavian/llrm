#!/bin/bash
# usage: build.sh CC1 DIR LEVEL   (LEVEL = O2 | Os): compile compiler-rt's f32 sources with the z80 cc1 and assemble them into DIR
# (generic C) and DIR/x6502 (the 6502 config's knobs, mul/div from config/6502/sf32_6502.c).
set -e
here=$(cd "$(dirname "$0")" && pwd); S=$here/../../../runtime/compiler-rt
CC1=$1; O=$2; L=$3; mkdir -p "$O/x6502"; ulimit -v 16000000
F="add_sub mul div pack float_si float_usi fix_si fix_usi"
cc() { # SRC OUT [extra flags]
  local src=$1 out=$2; shift 2
  timeout 300 "$CC1" -quiet -"$L" -I"$here/inc" -I"$S/include" -DSF_INTABI "$@" "$src" -o "$out.s" || { echo "cc1 failed $src $L"; exit 1; }
  sdasz80 -o "$out.rel" "$out.s"
}
for f in $F; do cc "$S/lib/fp/f32/$f.c" "$O/f32_$f"; done
for f in $F; do [ "$f" = mul ] || [ "$f" = div ] || cc "$S/lib/fp/f32/$f.c" "$O/x6502/f32_$f" -include "$S/config/6502/config.h"; done
cc "$S/config/6502/sf32_6502.c" "$O/x6502/sf32_6502" -include "$S/config/6502/config.h"
cc "$S/lib/int/int32.c" "$O/int32"
