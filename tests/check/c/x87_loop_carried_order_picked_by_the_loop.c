// RUN: llrm-cc %s -m32 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// recfloat at -O2 m32: the loop's carried values (acc, x) entered in the order that made it exchange them on every trip (`fadd st(0), st(1)`,
// `fxch`, ..., `fxch`: 2 `fxch` a trip). The order the header takes is tried both ways and kept by the weighted length: one `fxch` before the loop,
// `fadd st(1), st(0)` in it, none a trip.
// CHECK-LABEL: bench_recfloat_ proc
// CHECK: fxch st(1)
// CHECK: fadd st(1), st(0)
// CHECK-NOT: fxch
// CHECK: jne
static double decay(double x, double acc, int n)
{
    if (n == 0) return acc;
    return decay(x * 0.5 + 1.0, acc + x, n - 1);
}

long bench_recfloat(int n)
{
    return (long)(decay(100.0, 0.0, n) * 1000.0 + 0.5);
}
