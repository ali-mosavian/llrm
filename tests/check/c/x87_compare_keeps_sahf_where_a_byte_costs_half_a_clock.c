// RUN: llrm-cc %s -m16 -march=i386 -O2 --isel=dag -S -o /dev/stdout
// DEVIATE X87-FCMP-AH prices its two extra bytes at the speed rate: m16's is 500 milliclocks a byte (1 clk = 2 B),
// so the clock `test ah, mask` saves over `sahf` is a tie and `sahf` stays (QCport pl_move -O2 +40 B).
// CHECK-LABEL: _f proc
// CHECK: fnstsw
// CHECK: sahf
// CHECK-NOT: test ah
double f(double a, double b) { return a > b ? a : b; }
