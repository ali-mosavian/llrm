extern void report(long value);

static int a[256];
long bench_x_insertion(int n)
{
    unsigned s = 777;
    int i, j, v; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1664525u + 1013904223u; a[i] = (int)(s >> 20); }
    for (i = 1; i < n; ++i) {
        v = a[i];
        for (j = i - 1; j >= 0 && a[j] > v; --j) a[j + 1] = a[j];
        a[j + 1] = v;
    }
    for (i = 0; i < n; ++i) sum = sum * 17 + a[i];
    return sum;
}

int main(void)
{
    report(bench_x_insertion(150));
    return 0;
}
