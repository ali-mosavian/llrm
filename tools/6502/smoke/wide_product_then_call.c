// flags: -fno-inline-functions
// Four 16x16 products make a 32x32 -> 64 multiply; its high word is read after the later products' calls, then a 32-bit divide follows.
extern void report(long value);

static unsigned long long u(unsigned long a, unsigned long b)
{
    return (unsigned long long)a * b;
}

static long long s(long a, long b)
{
    return (long long)a * b;
}

static unsigned long in[][2] = {
    {0xDEADBEEFUL, 0x12345678UL}, {3000000007UL, 70001UL}, {4294967295UL, 4294967295UL}, {65537UL, 255UL},
};

int main(void)
{
    int i;
    for (i = 0; i < 4; ++i) {
        volatile unsigned long *v = in[i];
        unsigned long long p = u(v[0], v[1]);
        long long q = s((long)v[0], (long)v[1]);
        report((long)p);
        report((long)(p >> 32));
        report((long)q);
        report((long)(q >> 32));
        report((long)(p / (v[1] | 1)));
    }
    return 0;
}
