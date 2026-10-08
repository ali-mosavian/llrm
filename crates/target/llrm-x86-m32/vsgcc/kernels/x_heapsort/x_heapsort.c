extern void report(long value);

static int a[256];
static void sift(int lo, int hi)
{
    int root = lo, child, t;
    while ((child = 2 * root + 1) <= hi) {
        if (child + 1 <= hi && a[child] < a[child + 1]) ++child;
        if (a[root] >= a[child]) return;
        t = a[root]; a[root] = a[child]; a[child] = t; root = child;
    }
}
long bench_x_heapsort(int n)
{
    unsigned s = 4242; int i, t; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 22695477u + 1u; a[i] = (int)(s >> 17); }
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
