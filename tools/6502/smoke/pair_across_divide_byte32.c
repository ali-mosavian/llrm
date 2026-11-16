// flags: -fno-inline-functions
// A byte-divisor quotient-and-remainder pair (rc4..rc7 hold the remainder), then two wide divides: the first remainder is read after both.
extern void report(long value);

static long s(long x, long a, long b)
{
    long q = x / 7, r = x % 7, p = a / b, m = a % b;
    return ((p ^ r) + q) - m;
}

static unsigned long u(unsigned long x, unsigned long a, unsigned long b)
{
    unsigned long q = x / 200, r = x % 200, p = a / b, m = a % b;
    return ((p ^ r) + q) - m;
}

static long in[][3] = {
    {-482900001L, 123456789L, -70000L},
    {482900001L, -2000000000L, 100L},
    {2000000000L, 99999L, 0x01020304L},
    {-1L, -25536L, 9L},
    {123456L, -987654321L, 255L},
};

int main(void)
{
    int i;
    for (i = 0; i < 5; ++i) {
        volatile long *v = in[i];
        report(s(v[0], v[1], v[2]));
        report((long)u((unsigned long)v[0], (unsigned long)v[1], (unsigned long)v[2]));
    }
    return 0;
}
