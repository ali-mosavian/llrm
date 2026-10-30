// RUN: llrm-cc %s -m16 -mabi=ia16 -march=i386 -Os --isel=dag -S -o /dev/stdout
// `big + 20000` (80000 bytes, past a segment) was computed at run time, `movzx; add 80000; sar 16; shl 12`; an object that long starts its segment, so its end is `offset _big+14464` and `seg _big` + 4096.
long __huge big[20000];
int below_end(long __huge *p) { return p < big + 20000; }
// CHECK-LABEL: _below_end proc
// CHECK-NOT: sar
// CHECK: offset _big+14464
