extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static u32 a[256], b[256];
long bench_x_radix(i32 n)
{
    u32 s = 31337; i32 i, pass; long sum = 0;
    for (i = 0; i < n; ++i) { s = s * 1103515245ul + 12345ul; a[i] = s >> 8; }
    for (pass = 0; pass < 3; ++pass) {
        u32 count[256]; i32 shift = pass * 8, d;
        for (d = 0; d < 256; ++d) count[d] = 0;
        for (i = 0; i < n; ++i) ++count[(a[i] >> shift) & 255];
        { u32 t = 0; for (d = 0; d < 256; ++d) { u32 c = count[d]; count[d] = t; t += c; } }
        for (i = 0; i < n; ++i) b[count[(a[i] >> shift) & 255]++] = a[i];
        for (i = 0; i < n; ++i) a[i] = b[i];
    }
    for (i = 0; i < n; ++i) sum = sum * 7 + (long)(a[i] & 0xFFFF);
    return sum;
}

int main(void)
{
    report(bench_x_radix(200));
    return 0;
}
