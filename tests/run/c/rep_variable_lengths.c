// flags: -O2 -march=i486 -m32 | -Os -march=i486 -m32
// targets: dos
/* A copy and a fill whose length is known only at run time (rep movsb / rep stosb outside loops at -Os):
   lengths 0, 1, 3, 4, 5, 255 and 256 at odd offsets, with bytes either side of the block checked. */
extern void report(long value);

static unsigned char s[300], d[300];

static long sum(void)
{
    unsigned long h = 0;
    unsigned short i;
    for (i = 0; i < 300; ++i) h = h * 31 + d[i];
    return (long)(h & 0x7fffffffUL);
}

static void init(void)
{
    unsigned short i;
    for (i = 0; i < 300; ++i) { s[i] = (unsigned char)(i * 7 + 1); d[i] = 100; }
}

__attribute__((noinline)) static void copy(unsigned short n) { __builtin_memcpy(d + 3, s + 5, n); }
__attribute__((noinline)) static void fill(unsigned short n, int v) { __builtin_memset(d + 3, v, n); }

int main(void)
{
    static const unsigned short lens[] = {0, 1, 3, 4, 5, 255, 256};
    volatile unsigned short nn;
    unsigned short k;
    for (k = 0; k < sizeof lens / sizeof lens[0]; ++k) {
        nn = lens[k];
        init(); copy(nn); report(sum());
        init(); fill(nn, 0); report(sum());
        init(); fill(nn, 0xa5); report(sum());
    }
    return 0;
}
