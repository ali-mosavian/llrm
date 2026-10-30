extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static u32 table[256];
long bench_x_crc32(i32 n)
{
    u32 c, crc = 0xFFFFFFFFul; i32 i, k;
    for (i = 0; i < 256; ++i) { c = (u32)i; for (k = 0; k < 8; ++k) c = (c & 1) ? 0xEDB88320ul ^ (c >> 1) : c >> 1; table[i] = c; }
    for (i = 0; i < n; ++i) crc = table[(crc ^ (u32)(i * 7 + (i >> 3))) & 255] ^ (crc >> 8);
    return (long)(crc & 0x7FFFFFFF);
}

int main(void)
{
    report(bench_x_crc32(2000));
    return 0;
}
