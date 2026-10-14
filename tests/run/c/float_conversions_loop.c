// flags: -O2
// A loop of `(long)a[i]` runs in the truncating rounding mode throughout (the switch hoisted to its entry and exit), and
// the caller's mode is back after it: 1.0/10.0 computed afterwards rounds to nearest (low word 0x9999999A), not toward
// zero (0x99999999). The conversions truncate: 1.9 is 1, -1.9 is -1.
extern void report(long value);

static double a[8] = {1.9, -1.9, 2.5, -2.5, 3.99, 0.1, 100.7, -0.9};

static long sum_truncated(int n)
{
    long sum = 0;
    int i;

    for (i = 0; i < n; ++i)
        sum += (long)a[i];
    return sum;
}

int main(void)
{
    volatile double one = 1.0, ten = 10.0;
    union {
        double d;
        unsigned long w[2];
    } quotient;

    report(sum_truncated(8));
    quotient.d = one / ten;
    report((long)(quotient.w[0] & 0xFFFFFFL));
    report(sum_truncated(3));
    return 0;
}
