extern void report(long value);

static unsigned char data[2048]; static int hist[64];
long bench_x_histo2(int n)
{
    int i, max = 0, at = 0; unsigned s = 77;
    for (i = 0; i < n; ++i) { s = s * 1664525u + 1013904223u; data[i] = (unsigned char)(s >> 24); }
    for (i = 0; i < n; ++i) ++hist[(data[i] >> 2) & 63];
    for (i = 0; i < 64; ++i) if (hist[i] > max) { max = hist[i]; at = i; }
    return max * 100 + at;
}

int main(void)
{
    report(bench_x_histo2(1500));
    return 0;
}
