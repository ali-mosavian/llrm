extern void report(long value);

static int pop(unsigned x) { int c = 0; while (x) { x &= x - 1; ++c; } return c; }
static unsigned rev32(unsigned x) { x = ((x >> 1) & 0x55555555u) | ((x & 0x55555555u) << 1); x = ((x >> 2) & 0x33333333u) | ((x & 0x33333333u) << 2); x = ((x >> 4) & 0x0F0F0F0Fu) | ((x & 0x0F0F0F0Fu) << 4); x = ((x >> 8) & 0x00FF00FFu) | ((x & 0x00FF00FFu) << 8); return (x >> 16) | (x << 16); }
long bench_x_popcount(int n)
{
    unsigned s = 1; int i; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1664525u + 1013904223u; sum += pop(s) + (rev32(s) & 255); }
    return sum;
}

int main(void)
{
    report(bench_x_popcount(2000));
    return 0;
}
