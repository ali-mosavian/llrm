extern void report(long value);

long bench_x_iir(int n)
{
    int i; long y1 = 0, y2 = 0, sum = 0; unsigned s = 1;
    for (i = 0; i < n; ++i) {
        long x, y; s = s * 1664525u + 1013904223u; x = (long)((s >> 20) & 0x3FF) - 512;
        y = (x * 1024 + y1 * 1600 - y2 * 700) >> 10;
        y2 = y1; y1 = y; sum += y;
    }
    return sum;
}

int main(void)
{
    report(bench_x_iir(1500));
    return 0;
}
