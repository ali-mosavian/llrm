#!/bin/sh
# Control for reach32.py: a calls b, c is unreferenced and must not count; with c called from a it must.
set -e
cd "$(dirname "$0")"; T=$(mktemp -d)
mk() { printf '__attribute__((noinline)) int b(int x) { return x * 3 + 1; }\n__attribute__((noinline)) int c(int x) { return x * 5 + 7; }\n__attribute__((noinline)) int a(int x) { return b(x) + %s; }\nvoid _start(void) {}\n' "$1" > $T/t.c
  gcc -m32 -O1 -fno-pic -ffreestanding -c $T/t.c -o $T/t.o && ld -m elf_i386 -e _start -o $T/t.elf $T/t.o
  python3 reach32.py $T/t.elf a | cut -d' ' -f1; }
sz() { nm -S $T/t.elf | awk -v n=$1 '$4==n{print strtonum("0x"$2)}' 2>/dev/null || true; }
r1=$(mk "1"); s_a=$(nm -S $T/t.elf | grep ' a$' | cut -d' ' -f2); s_b=$(nm -S $T/t.elf | grep ' b$' | cut -d' ' -f2); s_c=$(nm -S $T/t.elf | grep ' c$' | cut -d' ' -f2)
want=$((0x$s_a + 0x$s_b)); [ "$r1" = "$want" ] || { echo "FAIL reached $r1, want $want (a+b)"; exit 1; }
r2=$(mk "c(x)"); s_a=$(nm -S $T/t.elf | grep ' a$' | cut -d' ' -f2); s_b=$(nm -S $T/t.elf | grep ' b$' | cut -d' ' -f2); s_c=$(nm -S $T/t.elf | grep ' c$' | cut -d' ' -f2)
want=$((0x$s_a + 0x$s_b + 0x$s_c)); [ "$r2" = "$want" ] || { echo "FAIL with c called: $r2, want $want"; exit 1; }
echo "ok: $r1 B (a+b), $r2 B when a calls c"; rm -r $T
