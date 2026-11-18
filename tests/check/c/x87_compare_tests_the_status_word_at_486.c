// RUN: llrm-cc %s --target=x86_32-wat -mabi=sysv -march=i486 -O2 -S -o /dev/stdout
// DEVIATE X87-FCMP-AH (-O2, -O3). One flag reader after an x87 compare reads the status word: `test ah, mask` is 1 clock
// on a 486, `sahf` 2 (x_ld_abs: 256 sahf, +239 clk against gcc, which tests ah). Two readers share one `sahf`.
// CHECK-LABEL: _f proc
// CHECK: fnstsw ax
// CHECK-NEXT: test ah, 65
// CHECK-NEXT: jne
// CHECK-NOT: sahf
// CHECK-LABEL: _g proc
// CHECK: fnstsw ax
// CHECK-NEXT: test ah, 1
// CHECK-NEXT: sete cl
// CHECK-NOT: sahf
// CHECK-LABEL: _h proc
// CHECK: fnstsw ax
// CHECK-NEXT: sahf
// CHECK-NEXT: setnp cl
double f(double a, double b) { return a > b ? a : b; }
int g(double a, double b) { return a >= b; }
int h(double a, double b) { return a == b; }
