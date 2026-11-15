// flags: -fno-inline-functions
// A 32-bit product live across a 32-bit divide pair, and a product of the results after it.
extern void report(long value);

static long f(long x, long y, long z, long w)
{
    long t = x * z;
    long q = x / y, r = x % y;
    return t + q * w + r * z;
}

static unsigned long g(unsigned long x, unsigned long y, unsigned long z, unsigned long w)
{
    unsigned long t = x * z;
    unsigned long q = x / y, r = x % y;
    return t - q * w + r * z;
}

static long in[][4] = {
    {-482900001L, 3707L, 33L, 5L},
    {2000000000L, 70001L, 300111L, -11L},
    {123456789L, 255L, -257L, 65535L},
    {-1L, 1000003L, 7L, 70000L},
};

int main(void)
{
    int i;
    for (i = 0; i < 4; ++i) {
        volatile long *v = in[i];
        report(f(v[0], v[1], v[2], v[3]));
        report((long)g((unsigned long)v[0], (unsigned long)v[1], (unsigned long)v[2], (unsigned long)v[3]));
    }
    return 0;
}
