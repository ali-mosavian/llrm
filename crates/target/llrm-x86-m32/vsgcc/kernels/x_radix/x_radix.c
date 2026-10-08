extern void report(long value);

static unsigned a[256], b[256];
long bench_x_radix(int n)
{
    unsigned s = 31337; int i, pass; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1103515245u + 12345u; a[i] = s >> 8; }
    for (pass = 0; pass < 3; ++pass) {
        unsigned count[256]; int shift = pass * 8, d;
        for (d = 0; d < 256; ++d) count[d] = 0;
        for (i = 0; i < n; ++i) ++count[(a[i] >> shift) & 255];
        { unsigned t = 0; for (d = 0; d < 256; ++d) { unsigned c = count[d]; count[d] = t; t += c; } }
        for (i = 0; i < n; ++i) b[count[(a[i] >> shift) & 255]++] = a[i];
        for (i = 0; i < n; ++i) a[i] = b[i];
    }
    for (i = 0; i < n; ++i) sum = sum * 7 + (long)(a[i] & 0xFFFF);
    return sum;
}

int main(void)
{
    report(bench_x_radix(200));
    return 0;
}
