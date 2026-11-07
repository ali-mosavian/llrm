// RUN: llrm-cc %s -m32 -O2 -march=i486 --isel=dag -S -o /dev/stdout
// recfloat at -O2 m32: the loop's two float chains (`x * 0.5 + 1`, `acc + x`) in the order that updates acc in
// place leave no copy of x: 8 instructions a trip, not 9. The other order copied x (`fld st(1)`).
// CHECK-LABEL: bench_recfloat_ proc
// CHECK: fadd st(0), st(1)
// CHECK-NOT: fld st
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
