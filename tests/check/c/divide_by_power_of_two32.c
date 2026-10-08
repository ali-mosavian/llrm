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
// The edges: the reciprocal is exact and normal from 2^-126 to 2^126 (float) and 2^-1022 to 2^1022 (double), either sign.
float f_max(float x) { return x / 85070591730234615865843651857942052864.0f; }
float f_min(float x) { return x / 1.1754943508222875e-38f; }
float f_neg(float x) { return x / -4.0f; }
double d_max(double x) { return x / 44942328371557897693232629769725618340449424473557664318357520289433168951375240783177119330601884005280028469967848339414697442203604155623211857659868531094441973356216371319075554900311523529863270738021251442209537670585615720368478277635206809290837627671146574559986811484619929076208839082406056034304.0; }
long double x_neg(long double x) { return x / -8.0L; }
// CHECK-LABEL: _f_max proc
// CHECK-NOT: fdiv
// CHECK: fmul
// CHECK: _f_max endp
// CHECK-LABEL: _f_min proc
// CHECK-NOT: fdiv
// CHECK: fmul
// CHECK: _f_min endp
// CHECK-LABEL: _f_neg proc
// CHECK-NOT: fdiv
// CHECK: fmul
// CHECK: _f_neg endp
// CHECK-LABEL: _d_max proc
// CHECK-NOT: fdiv
// CHECK: fmul
// CHECK: _d_max endp
// CHECK-LABEL: _x_neg proc
// CHECK-NOT: fdiv
// CHECK: fmul
// CHECK: _x_neg endp
