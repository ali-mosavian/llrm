extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static i32 a[256], b[256];
static void msort(i32 lo, i32 hi)
{
    i32 mid, i, j, k;
    if (hi - lo < 2) return;
    mid = (lo + hi) / 2;
    msort(lo, mid); msort(mid, hi);
    i = lo; j = mid; k = lo;
    while (i < mid && j < hi) b[k++] = a[i] <= a[j] ? a[i++] : a[j++];
    while (i < mid) b[k++] = a[i++];
    while (j < hi) b[k++] = a[j++];
    for (k = lo; k < hi; ++k) a[k] = b[k];
}
long bench_x_mergesort(i32 n)
{
    u32 s = 99; i32 i; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 69069ul + 1ul; a[i] = (i32)(s >> 18); }
    msort(0, n);
    for (i = 0; i < n; ++i) sum = sum * 11 + a[i];
    return sum;
}

int main(void)
{
    report(bench_x_mergesort(200));
    return 0;
}
