// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=i486 -O1 -S -o /dev/stdout
// DEVIATE X87-FCMP-AH is not made at -O1: bytes are held there, and `test ah, mask` is 2 bytes longer than `sahf`
// (x_ld_cmp3 -O1 was +2 B over clang with the test, 184 against 182).
// CHECK-LABEL: _f proc
// CHECK: fnstsw ax
// CHECK-NEXT: sahf
// CHECK-NOT: test ah
double f(double a, double b) { return a > b ? a : b; }
