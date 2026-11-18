// flags: -fno-inline-functions
// The same 32-bit arithmetic in four functions: the module outlines the repeated instruction sequences as called routines (one body, a
// call in each function), and a carry produced inside one is read after it. The answers are the host's.
extern void report(long value);

#define BODY \
    unsigned long a = x + y; \
    unsigned long c = a < x; \
    unsigned long b = (a ^ (y << 3)) + (x >> 5); \
    unsigned long d = b - y; \
    unsigned long e = d + (c << 16) + (a >> 9); \
    return (long)(e ^ (b + c));

static long f1(unsigned long x, unsigned long y) { BODY }
static long f2(unsigned long x, unsigned long y) { BODY }
static long f3(unsigned long x, unsigned long y) { BODY }

static long g(long x, long y)
{
    unsigned long a = (unsigned long)x + (unsigned long)y;
    unsigned long b = (a ^ ((unsigned long)y << 3)) + (unsigned long)(x >> 5);
    unsigned long d = b - (unsigned long)y;
    return (long)((d + (unsigned long)((long)a >> 9)) ^ (b + (a < (unsigned long)x)));
}

static unsigned long in[][2] = {
    {0xFFFFFFF0UL, 0x20UL},
    {0x80000000UL, 0x80000000UL},
    {0x12345678UL, 0x9ABCDEF0UL},
    {1UL, 0xFFFFFFFFUL},
    {0UL, 0UL},
};

int main(void)
{
    int i;
    for (i = 0; i < 5; ++i) {
        volatile unsigned long *v = in[i];
        unsigned long x = v[0], y = v[1];
        report(f1(x, y));
        report(f2(y, x));
        report(f3(x + 1, y));
        report(g((long)x, (long)y));
    }
    return 0;
}
