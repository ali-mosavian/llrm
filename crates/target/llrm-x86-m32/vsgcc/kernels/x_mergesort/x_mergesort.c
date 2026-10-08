extern void report(long value);

static int a[256], b[256];
static void msort(int lo, int hi)
{
    int mid, i, j, k;
    if (hi - lo < 2) return;
    mid = (lo + hi) / 2;
    msort(lo, mid); msort(mid, hi);
    i = lo; j = mid; k = lo;
    while (i < mid && j < hi) b[k++] = a[i] <= a[j] ? a[i++] : a[j++];
    while (i < mid) b[k++] = a[i++];
    while (j < hi) b[k++] = a[j++];
    for (k = lo; k < hi; ++k) a[k] = b[k];
}
long bench_x_mergesort(int n)
{
    unsigned s = 99; int i; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 69069u + 1u; a[i] = (int)(s >> 18); }
    msort(0, n);
    for (i = 0; i < n; ++i) sum = sum * 11 + a[i];
    return sum;
}

int main(void)
{
    report(bench_x_mergesort(200));
    return 0;
}
