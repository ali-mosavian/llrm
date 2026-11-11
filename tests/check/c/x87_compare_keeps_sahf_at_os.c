// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=i486 -Os -S -o /dev/stdout
// DEVIATE X87-FCMP-AH is not made for size: `test ah, mask` is 3 bytes, `sahf` 1.
// CHECK-LABEL: _f proc
// CHECK: fnstsw ax
// CHECK-NEXT: sahf
// CHECK-NOT: test ah
double f(double a, double b) { return a > b ? a : b; }
