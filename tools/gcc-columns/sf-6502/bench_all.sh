#!/bin/bash
# usage: bench_all.sh CC1 WORKDIR: every op, both variants, O2 and Os, on sim65, then the hand asm (about 5 minutes)
here=$(cd "$(dirname "$0")" && pwd); CC1=$1; W=$2; mkdir -p "$W"
echo "cc1 md5 $(md5sum "$CC1" | cut -c1-32)"
"$here/build.sh" "$CC1" "$W/o2" O2 && "$here/build.sh" "$CC1" "$W/os" Os || exit 1
for L in o2 os; do for V in generic x6502; do
  for spec in add:___addsf3 sub:___subsf3 mul:___mulsf3 div:___divsf3 itof:___floatsisf utof:___floatunsisf ftoi:___fixsfsi ftou:___fixunssfsi; do
    "$here/run.sh" "$W/$L" $V "${spec%%:*}" "${spec#*:}" 2>&1 | tail -1
  done
done; done
"$here/hand.sh"
echo DONE
