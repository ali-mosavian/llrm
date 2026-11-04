#!/bin/sh
# usage: tests/z80/run_cmp_bool.sh [ops]   (slt sgt seq): ours wrapped to SDCC's bool contract. The wrapper (the instructions after the call that turn the
# compiler-rt int in DE into 0/1) is counted: it is in the library run and not in the stub run. Same vectors as bench_sdcc.sh fslt fsgt fseq.
set -e
cd "$(dirname "$0")"; OPS=${*:-slt sgt seq}; T=$(mktemp -d); S=../../config/z80
printf '\t.module crt0\n\t.globl _main\n\t.area _HEADER (ABS)\n\t.org 0\n\tld sp,#0\n\tcall _main\n\thalt\n\t.area _CODE\n\t.area _DATA\n' > $T/crt0.s
sdasz80 -o $T/crt0.rel $T/crt0.s
cp $S/sf32_cmp.s $T/sf32_cmp.s; sdasz80 -g -o $T/sf32_cmp.rel $T/sf32_cmp.s
for op in $OPS; do
  case $op in
    slt) f=__ltsf2; post='\tld\ta,d\n\trlca\n\tld\tde,#0\n\trl\te\n';;
    sgt) f=__gtsf2; post='\tld\thl,#0\n\tor\ta,a\n\tsbc\thl,de\n\tld\ta,h\n\trlca\n\tld\tde,#0\n\trl\te\n';;
    seq) f=__eqsf2; post='\tld\ta,d\n\tor\ta,e\n\tld\tde,#0\n\tjr\tnz,1$\n\tinc\te\n1$:\n';;
  esac
  python3 gen_cmp_bool.py $op > $T/vec.inc
  sed "s/OP/$f/g" drv16.s > $T/drv_s.s
  awk -v post="$(printf "$post")" '{print} /^\tcall\t/ && $2 == "'$f'" {print post}' $T/drv_s.s > $T/drv_r.s
  printf '\t.module stub\n\t.area _CODE\n\t.globl %s\n%s:\n\tret\n' $f $f > $T/stub.s
  mkdir -p $T/b; python3 basevec.py 2 < $T/vec.inc > $T/b/vec.inc; cp $T/drv_s.s $T/b/drv_s.s
  (cd $T/b; sdasz80 -o d1.rel drv_s.s)
  (cd $T; sed 's/\.module drv/.module drv_s/' drv_s.s > d1.s; sed 's/\.module drv/.module drv_r/' drv_r.s > d2.s
   sdasz80 -o d2.rel d2.s; sdasz80 -o stub.rel stub.s
   sdldz80 -i -b _CODE=0x100 s.ihx crt0.rel b/d1.rel stub.rel >/dev/null
   sdldz80 -i -b _CODE=0x100 r.ihx crt0.rel d2.rel sf32_cmp.rel >/dev/null)
  run() { printf 'run\nstate\ndump rom 0x8f00 0x8f01\nquit\n' | ucsim_z80 -t Z80 -q -w $T/$1.ihx -c - 2>&1; }
  os=$(run s); b=$(echo "$os" | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
  fb=$(echo "$os" | sed -n 's/^0x8f00 *\([0-9a-f][0-9a-f]\) *\([0-9a-f][0-9a-f]\).*/\2\1/p' | head -1)
  [ "${fb:-ffff}" = 0000 ] || { echo "$op: the stub run must pass its records (basevec.py), got $fb"; exit 1; }
  o=$(run r); c=$(echo "$o" | sed -n 's/.*(\([0-9]*\) clks).*/\1/p' | head -1)
  f2=$(echo "$o" | sed -n 's/^0x8f00 *\([0-9a-f][0-9a-f]\) *\([0-9a-f][0-9a-f]\).*/\2\1/p' | head -1)
  nv=$(sed -n 's/^NV = //p' $T/vec.inc)
  echo "$op wrapped: fails=$((0x${f2:-ffff})) cycles=$(( (c-b)/nv ))"
done
rm -r $T
