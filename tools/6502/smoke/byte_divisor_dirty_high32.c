// flags: -fno-inline-functions
// A divisor known to be 1 to 255 reads only its low byte: a wide divide first leaves its remainder in every cell
// of the divisor's range, then byte divisors (a constant, a masked variable; signed, unsigned) divide and take the remainder.
extern void report(long value);

static long s(long a, long b, long x, long y)
{
    long q = a / b, r = a % b;
    return (q ^ r) + x / 5 + y % ((b & 127) + 1) + x % 7;
}

static unsigned long u(unsigned long a, unsigned long b, unsigned long x, unsigned long y)
{
    unsigned long q = a / b, r = a % b;
    return (q ^ r) + x / 9 + y % ((b & 255) | 1) + x % 250;
}

static long in[][4] = {
    {-482900001L, 0x01020304L, 123456789L, -70000L},
    {482900001L, -0x7F7F7F01L, -2000000000L, 100L},
    {2000000000L, 0x00FF0001L, 99999L, 3L},
    {-1L, 0x7FFFFFFFL, -25536L, 9L},
    {123456L, -0x01010101L, -987654321L, 255L},
};

int main(void)
{
    int i;
    for (i = 0; i < 5; ++i) {
        volatile long *v = in[i];
        report(s(v[0], v[1], v[2], v[3]));
        report((long)u((unsigned long)v[0], (unsigned long)v[1], (unsigned long)v[2], (unsigned long)v[3]));
    }
    return 0;
}
