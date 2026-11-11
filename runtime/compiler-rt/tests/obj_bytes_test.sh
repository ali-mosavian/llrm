#!/bin/sh
# obj_bytes must not count .eh_frame: llrm-cc objects carry 48 B of it per object, which inflated every size_x86.sh
# column but the gcc ones (a one-instruction negdf2 read 78 B against libgcc's 29).
cd "$(dirname "$0")"; . ./obj_bytes.sh
T=$(mktemp -d); trap 'rm -r $T' EXIT
printf 'int f(int a) { return -a; }\n' > $T/a.c
gcc -m32 -Os -fno-pic -fasynchronous-unwind-tables -c $T/a.c -o $T/a.o
text=$(size -A $T/a.o | awk '$1==".text"{print $2}'); eh=$(size -A $T/a.o | awk '$1==".eh_frame"{print $2}')
[ "${eh:-0}" -gt 0 ] || { echo "FAIL: the probe object has no .eh_frame"; exit 1; }
got=$(obj_bytes $T/a.o)
[ "$got" = "$text" ] || { echo "FAIL: obj_bytes $got, .text alone $text (.eh_frame $eh counted)"; exit 1; }
echo "obj_bytes ok: $got B, .eh_frame $eh B not counted"
