extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static i32 pop(u32 x) { i32 c = 0; while (x) { x &= x - 1; ++c; } return c; }
static u32 rev32(u32 x) { x = ((x >> 1) & 0x55555555ul) | ((x & 0x55555555ul) << 1); x = ((x >> 2) & 0x33333333ul) | ((x & 0x33333333ul) << 2); x = ((x >> 4) & 0x0F0F0F0Ful) | ((x & 0x0F0F0F0Ful) << 4); x = ((x >> 8) & 0x00FF00FFul) | ((x & 0x00FF00FFul) << 8); return (x >> 16) | (x << 16); }
long bench_x_popcount(i32 n)
{
    u32 s = 1; i32 i; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1664525ul + 1013904223ul; sum += pop(s) + (rev32(s) & 255); }
    return sum;
}

int main(void)
{
    report(bench_x_popcount(2000));
    return 0;
}
