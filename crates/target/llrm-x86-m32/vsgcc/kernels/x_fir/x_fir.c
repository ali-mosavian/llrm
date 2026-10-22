extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static short x[512], coef[16];
long bench_x_fir(i32 n)
{
    i32 i, k; long sum = 0; u32 s = 17;
    for (i = 0; i < 16; ++i) coef[i] = (short)(i * 100 - 700);
    for (i = 0; i < n + 16; ++i) { s = s * 1103515245ul + 12345ul; x[i] = (short)((s >> 16) & 0xFFF); }
    for (i = 0; i < n; ++i) { long acc = 0; for (k = 0; k < 16; ++k) acc += (long)x[i + k] * coef[k]; sum += acc >> 8; }
    return sum;
}

int main(void)
{
    report(bench_x_fir(400));
    return 0;
}
