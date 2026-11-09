// RUN: llrm-cc %s -m32 -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// DEVIATE TA-10 at -Os: a byte add on a value that stays live (the shift count `n - 8` and `n`) is one `lea`, not `mov ecx, edx; add cl, 24`
// (5 B for 3). It was dropped at -Os while 11 m32 cells grew; on the current tip 15 get smaller, 3 larger (rows TA10-*).
unsigned f(unsigned a, unsigned n) { return (a << ((n - 8) & 31)) + n; }
// CHECK-LABEL: _f proc
// CHECK: lea ecx, [{{e[a-d]x}}+24]
// CHECK-NOT: add cl
// CHECK: _f endp
