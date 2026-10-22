extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static i32 a[200];
long bench_x_bubble(i32 n)
{
    u32 s = 12345;
    i32 i, j, t; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1103515245ul + 12345ul; a[i] = (i32)((s >> 16) & 0x7FFF); }
    for (i = 0; i < n - 1; ++i)
        for (j = 0; j < n - 1 - i; ++j)
            if (a[j] > a[j + 1]) { t = a[j]; a[j] = a[j + 1]; a[j + 1] = t; }
    for (i = 0; i < n; ++i) sum = sum * 31 + a[i];
    return sum;
}

int main(void)
{
    report(bench_x_bubble(120));
    return 0;
}
