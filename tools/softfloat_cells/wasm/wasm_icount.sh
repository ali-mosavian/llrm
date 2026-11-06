#!/bin/bash
# usage: wasm_icount.sh [RT=runtime/compiler-rt] [N=300] -> executed wasm instructions per call at -O2, ours (config/wasm) vs the pin's compiler-rt (-D__SOFTFP__), wasm32.
# Counter: wasm-interp --trace, one '#' line per executed op; the driver and call cost cancel against a null function of the same signature (positive control:
# a known-different function must differ, and the count must grow linearly with N). The interpreter's own pseudo-ops (alloca, drop_keep) are counted alike in both columns.
H=$(cd "$(dirname "$0")" && pwd); RT=${1:-$H/../../../runtime/compiler-rt}; N=${2:-300}; CRT=${CRT:-$HOME/scratch/llvm-src/compiler-rt/lib/builtins}
W=$(mktemp -d); trap 'rm -r "${W:?}"' EXIT; C=${CLANG:-/usr/bin/clang}; F="--target=wasm32 -O2 -w -ffreestanding -fno-builtin"
for t in wasm-interp wasm-ld $C; do command -v $t > /dev/null || { echo "wasm_icount: $t not found"; exit 2; }; done
cnt() { # kind nv fn objs... -> executed ops
  k=$1; nv=$2; fn=$3; shift 3
  $C $F -DKIND=$k -DNV=$nv -DLIBFN=$fn -c $H/drv.c -o $W/drv.o && wasm-ld --no-entry --export=go -o $W/x.wasm $W/drv.o "$@" 2> $W/lerr || { echo "link FAIL $(head -1 $W/lerr)" >&2; echo 0; return; }
  wasm-interp $W/x.wasm --run-all-exports --trace 2> /dev/null | grep -c '^#'; }
per() { # kind fn objs... -> ops per call, null subtracted
  k=$1; fn=$2; shift 2
  a0=$(cnt $k 0 $fn "$@"); an=$(cnt $k $N $fn "$@"); b0=$(cnt $k 0 nul$k $W/nul.o); bn=$(cnt $k $N nul$k $W/nul.o)
  awk -v a=$an -v b=$a0 -v c=$bn -v d=$b0 -v n=$N 'BEGIN{printf "%.1f", ((a-b)-(c-d))/n}'; }
$C $F -c $H/nul.c -o $W/nul.o; $C $F -c $H/stub.c -o $W/stub.o
printf 'typedef unsigned long long u64;\ndouble nulx(double a, double c) { union { double d; u64 u; } x = { a }; u64 v = x.u; for (int i = 0; i < 8; i++) v = v * 3 + (v >> 7) + i; x.u = v; return x.d; }\n' > $W/ctl.c; $C $F -c $W/ctl.c -o $W/ctl.o
ctl=$(per 1 nul1 $W/nul.o); [ "$ctl" = "0.0" ] || { echo "FAIL: null function against itself reads $ctl, not 0.0"; exit 1; }
pc=$(per 1 nulx $W/ctl.o); awk -v v=$pc 'BEGIN{exit !(v>=8)}' || { echo "FAIL: a known 8-iteration function reads $pc ops/call over null (positive control)"; exit 1; }
[ "$(cnt 1 $N nul1 $W/nul.o)" -gt 1000 ] || { echo "FAIL: the driver executed no ops (link or trace broken)"; exit 1; }
for spec in div:1:f64/div:__divdf3:divdf3 mul:1:f64/mul:__muldf3:muldf3 add:1:f64/add_sub:__adddf3:adddf3 sidf:2:f64/float_si:__floatsidf:floatsidf didf:3:f64/float_di:__floatdidf:floatdidf; do
  IFS=: read op kind src sym cfile <<< "$spec"
  $C $F -include stdint.h -include $RT/config/wasm/config.h -DSF_CONV_SPLIT ${EXTRA:-} -I$RT/include -c $RT/lib/$(echo fp/$src).c -o $W/o.o 2> /dev/null || { echo "$op: ours build FAIL"; continue; }
  $C $F -include stdint.h -include $RT/config/wasm/config.h -DSF_CONV_SPLIT -I$RT/include -c $RT/lib/fp/f64/pack.c -o $W/p.o
  $C $F -D__SOFTFP__ -I$CRT -c $CRT/$cfile.c -o $W/c.o 2> /dev/null || { echo "$op: crt build FAIL"; continue; }
  echo "wasm32 -O2 $op: ours $(per $kind $sym $W/o.o $W/p.o) crt $(per $kind $sym $W/c.o $W/stub.o) ops/call"
done
