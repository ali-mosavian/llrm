extern void report(long value);
typedef unsigned long u32;
typedef long i32; /* the width these kernels are written for: int is 16 bits at m16 */

static const char tab[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
static unsigned char in[300], out[500];
long bench_x_base64(i32 n)
{
    i32 i, o = 0; long sum = 0;
    for (i = 0; i < n; ++i) in[i] = (unsigned char)(i * 29 + 3);
    for (i = 0; i + 2 < n; i += 3) {
        u32 v = ((u32)in[i] << 16) | ((u32)in[i + 1] << 8) | in[i + 2];
        out[o++] = (unsigned char)tab[(v >> 18) & 63]; out[o++] = (unsigned char)tab[(v >> 12) & 63];
        out[o++] = (unsigned char)tab[(v >> 6) & 63]; out[o++] = (unsigned char)tab[v & 63];
    }
    for (i = 0; i < o; ++i) sum = sum * 3 + out[i];
    return sum;
}

int main(void)
{
    report(bench_x_base64(300));
    return 0;
}
