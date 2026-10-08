extern void report(long value);

static int a[200];
long bench_x_bubble(int n)
{
    unsigned s = 12345;
    int i, j, t; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1103515245u + 12345u; a[i] = (int)((s >> 16) & 0x7FFF); }
    for (i = 0; i < n - 1; ++i)
        for (j = 0; j < n - 1 - i; ++j)
            if (a[j] > a[j + 1]) { t = a[j]; a[j] = a[j + 1]; a[j + 1] = t; }
    for (i = 0; i < n; ++i) sum = sum * 31 + a[i];
    return sum;
}

int main(void)
{
    report(bench_x_bubble(120));
    return 0;
}
