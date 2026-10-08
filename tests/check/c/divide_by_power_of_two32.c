// RUN: llrm-c %s -m32 -mabi=sysv -O2 -march=i486 -S -o /dev/stdout
// A float divided by a power of two is multiplied by its reciprocal, exact: `fdiv` is 73 clocks on a 486, `fmul` 16.
// bench/fpbench divided twice per body by 16.0f, 124 clocks of 1.09 x gcc; neither gcc nor LLVM needs a flag for it.
float sixteenth(float x) { return x / 16.0f; }
double eighth(double x) { return x / 8.0; }
float third(float x) { return x / 3.0f; }
// 2^127: the reciprocal is subnormal, and inexact to multiply by.
float tiny(float x) { return x / 170141183460469231731687303715884105728.0f; }
// CHECK-LABEL: _sixteenth proc
// CHECK-NOT: fdiv
// CHECK: fmul
// CHECK: _sixteenth endp
// CHECK-LABEL: _eighth proc
// CHECK-NOT: fdiv
// CHECK: fmul
// CHECK: _eighth endp
// CHECK-LABEL: _third proc
// CHECK: fdiv
// CHECK: _third endp
// CHECK-LABEL: _tiny proc
// CHECK: fdiv
// CHECK: _tiny endp
