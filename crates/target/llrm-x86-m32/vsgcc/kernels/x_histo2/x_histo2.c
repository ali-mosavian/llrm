extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static unsigned char data[2048]; static i32 hist[64];
long bench_x_histo2(i32 n)
{
    i32 i, max = 0, at = 0; u32 s = 77;
    for (i = 0; i < n; ++i) { s = s * 1664525ul + 1013904223ul; data[i] = (unsigned char)(s >> 24); }
    for (i = 0; i < n; ++i) ++hist[(data[i] >> 2) & 63];
    for (i = 0; i < 64; ++i) if (hist[i] > max) { max = hist[i]; at = i; }
    return max * 100 + at;
}

int main(void)
{
    report(bench_x_histo2(1500));
    return 0;
}
