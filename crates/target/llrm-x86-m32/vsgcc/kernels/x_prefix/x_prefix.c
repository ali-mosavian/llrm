extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static i32 a[1024];
long bench_x_prefix(i32 n)
{
    i32 i, r; long sum = 0;
    for (i = 0; i < n; ++i) a[i] = (i * 5) & 15;
    for (r = 0; r < 4; ++r) { for (i = 1; i < n; ++i) a[i] += a[i - 1]; for (i = 0; i < n; ++i) a[i] &= 0xFFFF; }
    for (i = 0; i < n; ++i) sum += a[i];
    return sum;
}

int main(void)
{
    report(bench_x_prefix(600));
    return 0;
}
