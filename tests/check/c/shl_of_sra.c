// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 --isel=dag -S -o /dev/stdout
// (x+1) % 16 kept `sar 4; shl 4` (7 insns, con_print). The pin folds shl (sra x, c), c to and x, -16.
int f(int x) { return (x + 1) % 16; }
// CHECK-LABEL: _f proc
// CHECK: and {{.*}}, -16
// CHECK-NOT: shl
