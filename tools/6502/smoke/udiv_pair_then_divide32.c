// flags: -fno-inline-functions
// An unsigned 32-bit divide-and-remainder pair, then another 32-bit divide: the first remainder is read after the second divide.
extern void report(long value);

static unsigned long f(unsigned long x, unsigned long y, unsigned long a, unsigned long b)
{
    unsigned long q = x / y, r = x % y, p = a / b, m = a % b;
    return (p + r + q) ^ m;
}

static unsigned long in[][4] = {
    {4000000007UL, 3707UL, 123456789UL, 70000UL},
    {482900001UL, 37UL, 3000000000UL, 100UL},
    {2000000000UL, 70001UL, 99999UL, 3UL},
    {1UL, 255UL, 25536UL, 9UL},
    {123456UL, 1000003UL, 4294967295UL, 255UL},
};

int main(void)
{
    int i;
    for (i = 0; i < 5; ++i) {
        volatile unsigned long *v = in[i];
        report((long)f(v[0], v[1], v[2], v[3]));
    }
    return 0;
}
