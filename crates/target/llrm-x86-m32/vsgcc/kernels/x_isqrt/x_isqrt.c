extern void report(long value);

static unsigned isqrt(unsigned x) { unsigned r = 0, bit = 1u << 30; while (bit > x) bit >>= 2; while (bit) { if (x >= r + bit) { x -= r + bit; r = (r >> 1) + bit; } else r >>= 1; bit >>= 2; } return r; }
long bench_x_isqrt(int n)
{
    unsigned s = 3; int i; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1103515245u + 12345u; sum += isqrt(s >> 2); }
    return sum;
}

int main(void)
{
    report(bench_x_isqrt(1500));
    return 0;
}
