// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 -S -o /dev/stdout
// Twelve doubles a loop carries are live below its body: the scheduler counted none of them in the x87 stack's set, so the pressure it saw
// began at zero at the bottom and the order it chose needed 22 `fxch` a trip (86 lines). The pin counts every live-out value: 14 (75 lines).
// The same under-count cost nbody bq32 -O3 38.4k clocks of 426k once the stack was a set of seven.
extern double a[64], b[64];

// CHECK-LABEL: _kern proc
// CHECK: align 16
// CHECK-MAX-14: fxch
// CHECK: jne
double kern(int n)
{
    double x0 = 0, x1 = 1, x2 = 2, x3 = 3, x4 = 4, x5 = 5, x6 = 6, x7 = 7, x8 = 8, x9 = 9, x10 = 10, x11 = 11;
    for (int i = 0; i < n; i++) {
        double p = a[i], q = b[i];
        x0 += p * x1; x1 += q * x2; x2 += p * x3; x3 += q * x4; x4 += p * x5; x5 += q * x6;
        x6 += p * x7; x7 += q * x8; x8 += p * x9; x9 += q * x10; x10 += p * x11; x11 += q * x0;
    }
    return x0 + x1 + x2 + x3 + x4 + x5 + x6 + x7 + x8 + x9 + x10 + x11;
}
