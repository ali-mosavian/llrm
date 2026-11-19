// flags: -fno-inline-functions
// A signed 32-bit divide-and-remainder pair, then another 32-bit divide: the first remainder is read after the second divide.
extern void report(long value);

static long f(long x, long y, long a, long b)
{
    long q = x / y, r = x % y, p = a / b, m = a % b;
    return (p + r + q) ^ m;
}

static long in[][4] = {
    {-482900001L, 3707L, 123456789L, -70000L},
    {482900001L, -37L, -2000000000L, 100L},
    {2000000000L, 70001L, 99999L, 3L},
    {-1L, 255L, -25536L, 9L},
    {123456L, 1000003L, -987654321L, 255L},
};

int main(void)
{
    int i;
    for (i = 0; i < 5; ++i) {
        volatile long *v = in[i];
        report(f(v[0], v[1], v[2], v[3]));
    }
    return 0;
}
