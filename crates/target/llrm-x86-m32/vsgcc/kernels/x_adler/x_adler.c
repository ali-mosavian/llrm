extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

long bench_x_adler(i32 n)
{
    u32 a = 1, b = 0; i32 i;
    for (i = 0; i < n; ++i) { a += (u32)((i * 31 + 7) & 255); if (a >= 65521ul) a -= 65521ul; b += a; if (b >= 65521ul) b -= 65521ul; }
    return (long)((b << 16) | a) & 0x7FFFFFFF;
}

int main(void)
{
    report(bench_x_adler(3000));
    return 0;
}
