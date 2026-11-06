// RUN: llrm-cc %s -m32 -mabi=sysv -O2 -march=i486 -S -o /dev/stdout
// x_ld_max at m32 -O2: the pin full-unrolls the 8-trip outer loop, eight copies of the inner compare loop (232 -> 755 B) to save a counter and a branch a trip. The copies cost more bytes than the clocks pay (UP1): one loop stays.
// CHECK-LABEL: _bench proc
// CHECK: fcom
// CHECK-NOT: fcom
// CHECK: _bench endp
static long double x[64];
static long double ld_max(const long double *a, int n)
{
    long double m = a[0];
    for (int i = 1; i < n; i++)
        if (a[i] > m) m = a[i];
    return m;
}
long bench(int n)
{
    long double t = 0;
    for (int r = 0; r < 8; r++) {
        t += ld_max(x, n);
        x[r] += 1;
    }
    return (long)(t * 16);
}
