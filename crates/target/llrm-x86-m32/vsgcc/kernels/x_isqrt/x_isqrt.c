extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static u32 isqrt(u32 x) { u32 r = 0, bit = 1ul << 30; while (bit > x) bit >>= 2; while (bit) { if (x >= r + bit) { x -= r + bit; r = (r >> 1) + bit; } else r >>= 1; bit >>= 2; } return r; }
long bench_x_isqrt(i32 n)
{
    u32 s = 3; i32 i; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1103515245ul + 12345ul; sum += isqrt(s >> 2); }
    return sum;
}

int main(void)
{
    report(bench_x_isqrt(1500));
    return 0;
}
