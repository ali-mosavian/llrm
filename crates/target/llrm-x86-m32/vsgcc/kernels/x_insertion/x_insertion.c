extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static i32 a[256];
long bench_x_insertion(i32 n)
{
    u32 s = 777;
    i32 i, j, v; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1664525ul + 1013904223ul; a[i] = (i32)(s >> 20); }
    for (i = 1; i < n; ++i) {
        v = a[i];
        for (j = i - 1; j >= 0 && a[j] > v; --j) a[j + 1] = a[j];
        a[j + 1] = v;
    }
    for (i = 0; i < n; ++i) sum = sum * 17 + a[i];
    return sum;
}

int main(void)
{
    report(bench_x_insertion(150));
    return 0;
}
