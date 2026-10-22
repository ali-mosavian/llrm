extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

long bench_x_iir(i32 n)
{
    i32 i; long y1 = 0, y2 = 0, sum = 0; u32 s = 1;
    for (i = 0; i < n; ++i) {
        long x, y; s = s * 1664525ul + 1013904223ul; x = (long)((s >> 20) & 0x3FF) - 512;
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
