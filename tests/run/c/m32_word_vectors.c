// flags: -O2 | -O3
// targets: x86-m32
// The loops that gcc runs a word at a time (two 16-bit or four 8-bit lanes in one register) on a
// target with no vector registers: a bitwise operation over arrays, with a scalar or an array,
// a count the lane count does not divide, a start at an odd lane, and an in-place update.
extern void report(long value);

unsigned short a[67], b[67], c[67];
unsigned char p[131], q[131], s[131];

static void fill(void)
{
    long i;
    for (i = 0; i < 67; ++i) { a[i] = (unsigned short)(i * 997 + 13); b[i] = (unsigned short)(i * 331 + 7); c[i] = 0xa5a5; }
    for (i = 0; i < 131; ++i) { p[i] = (unsigned char)(i * 37 + 5); q[i] = (unsigned char)(i * 11 + 3); s[i] = 0x5a; }
}
static long sum16(const unsigned short *x, long n) { long i, t = 0; for (i = 0; i < n; ++i) t = t * 31 + x[i]; return t; }
static long sum8(const unsigned char *x, long n) { long i, t = 0; for (i = 0; i < n; ++i) t = t * 31 + x[i]; return t; }

__attribute__((noinline)) void xor16(unsigned short *d, const unsigned short *x, long n, long r)
{ long i; for (i = 0; i < n; ++i) d[i] = (unsigned short)(x[i] ^ r); }
__attribute__((noinline)) void and16(unsigned short *d, const unsigned short *x, const unsigned short *y, long n)
{ long i; for (i = 0; i < n; ++i) d[i] = (unsigned short)(x[i] & y[i]); }
__attribute__((noinline)) void or8(unsigned char *d, const unsigned char *x, long n, long r)
{ long i; for (i = 0; i < n; ++i) d[i] = (unsigned char)(x[i] | r); }
__attribute__((noinline)) void xor8(unsigned char *d, const unsigned char *x, const unsigned char *y, long n)
{ long i; for (i = 0; i < n; ++i) d[i] = (unsigned char)(x[i] ^ y[i]); }
__attribute__((noinline)) void inplace16(unsigned short *d, long n, long r)
{ long i; for (i = 0; i < n; ++i) d[i] ^= (unsigned short)r; }

int main(void)
{
    long n;
    static const long counts[] = {0, 1, 2, 3, 4, 5, 31, 64, 66, 67};
    for (n = 0; n < 10; ++n) {
        long m = counts[n];
        fill(); xor16(c, a, m, 0x1234 + n); report(sum16(c, 67));
        fill(); and16(c, a, b, m); report(sum16(c, 67));
        fill(); xor16(c + 1, a + 1, m > 0 ? m - 1 : 0, 0xfff0); report(sum16(c, 67));
        fill(); inplace16(a, m, 0x8001 + n); report(sum16(a, 67));
        if (m > 66) m = 66;
        fill(); or8(s, p, m * 2, 0x81 + n); report(sum8(s, 131));
        fill(); xor8(s, p, q, m * 2); report(sum8(s, 131));
        fill(); xor8(s + 3, p + 1, q + 2, m * 2 > 5 ? m * 2 - 5 : 0); report(sum8(s, 131));
    }
    return 0;
}
