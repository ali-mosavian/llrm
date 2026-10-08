extern void report(long value);

long bench_x_adler(int n)
{
    unsigned a = 1, b = 0; int i;
    for (i = 0; i < n; ++i) { a += (unsigned)((i * 31 + 7) & 255); if (a >= 65521u) a -= 65521u; b += a; if (b >= 65521u) b -= 65521u; }
    return (long)((b << 16) | a) & 0x7FFFFFFF;
}

int main(void)
{
    report(bench_x_adler(3000));
    return 0;
}
