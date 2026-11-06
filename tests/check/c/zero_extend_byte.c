// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -O2 --isel=dag -S -o /dev/stdout
// i486: `movzx r32,m8` takes 3 clocks (4 in 16-bit code), `xor r,r; mov r8,m8` two (three). DEVIATE ZL1 on the DAG
// path: textfill c -O2 m16 checksum loop, 4000 trips, +4000 clocks over main with the pin's movzx.
// CHECK-LABEL: {{_?total_?(@0)?}} proc
// CHECK: xor e{{[a-d]}}x, e{{[a-d]}}x
// CHECK: mov {{[a-d]l}}, byte ptr
// CHECK-NOT: movzx
// CHECK: {{_?total_?(@0)?}} endp
unsigned char b[64];
int total(void) { int i, s = 0; for (i = 0; i < 64; i++) s += b[i]; return s; }
