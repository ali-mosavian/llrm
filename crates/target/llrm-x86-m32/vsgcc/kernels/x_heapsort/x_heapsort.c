extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static i32 a[256];
static void sift(i32 lo, i32 hi)
{
    i32 root = lo, child, t;
    while ((child = 2 * root + 1) <= hi) {
        if (child + 1 <= hi && a[child] < a[child + 1]) ++child;
        if (a[root] >= a[child]) return;
        t = a[root]; a[root] = a[child]; a[child] = t; root = child;
    }
}
long bench_x_heapsort(i32 n)
{
    u32 s = 4242; i32 i, t; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 22695477ul + 1ul; a[i] = (i32)(s >> 17); }
    for (i = (n - 2) / 2; i >= 0; --i) sift(i, n - 1);
    for (i = n - 1; i > 0; --i) { t = a[0]; a[0] = a[i]; a[i] = t; sift(0, i - 1); }
    for (i = 0; i < n; ++i) sum = sum * 13 + a[i];
    return sum;
}

int main(void)
{
    report(bench_x_heapsort(200));
    return 0;
}
