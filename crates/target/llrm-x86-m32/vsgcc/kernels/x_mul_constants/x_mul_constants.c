// A multiply by a constant is shifts, adds and `lea` where the target prices them below `imul`: the same product as the multiply
// for every constant the chains make, over a spread of values, the wrap included.
extern void report(long value);

static unsigned long values[8] = {0UL, 1UL, 2UL, 0x7FFFUL, 0x8000UL, 0xFFFFFFFFUL, 0x80000000UL, 0x12345679UL};

#define SUM(K) static unsigned long sum##K(void)\
{\
    unsigned long total = 0, i, x = 0x9E3779B9UL;\
    for (i = 0; i < 8; ++i)\
        total += values[i] * K;\
    for (i = 0; i < 200; ++i) {\
        x = x * 1664525UL + 1013904223UL;\
        total = total * 31UL + x * K;\
    }\
    return total;\
}

SUM(3UL)
SUM(5UL)
SUM(9UL)
SUM(6UL)
SUM(10UL)
SUM(12UL)
SUM(17UL)
SUM(18UL)
SUM(20UL)
SUM(24UL)
SUM(25UL)
SUM(36UL)
SUM(40UL)
SUM(45UL)
SUM(72UL)
SUM(81UL)
SUM(100UL)
SUM(125UL)
SUM(127UL)
SUM(129UL)
SUM(255UL)
SUM(257UL)
SUM(1000UL)
SUM(65537UL)
SUM(7UL)
SUM(14UL)
SUM(15UL)
SUM(28UL)
SUM(30UL)
SUM(31UL)
SUM(62UL)
SUM(63UL)

long bench_x_mul_constants(int n)
{
    unsigned long t = 0;
    int k;
    for (k = 0; k < n; ++k) {
        t = t * 3UL + sum3UL();
        t = t * 3UL + sum5UL();
        t = t * 3UL + sum9UL();
        t = t * 3UL + sum6UL();
        t = t * 3UL + sum10UL();
        t = t * 3UL + sum12UL();
        t = t * 3UL + sum17UL();
        t = t * 3UL + sum18UL();
        t = t * 3UL + sum20UL();
        t = t * 3UL + sum24UL();
        t = t * 3UL + sum25UL();
        t = t * 3UL + sum36UL();
        t = t * 3UL + sum40UL();
        t = t * 3UL + sum45UL();
        t = t * 3UL + sum72UL();
        t = t * 3UL + sum81UL();
        t = t * 3UL + sum100UL();
        t = t * 3UL + sum125UL();
        t = t * 3UL + sum127UL();
        t = t * 3UL + sum129UL();
        t = t * 3UL + sum255UL();
        t = t * 3UL + sum257UL();
        t = t * 3UL + sum1000UL();
        t = t * 3UL + sum65537UL();
        t = t * 3UL + sum7UL();
        t = t * 3UL + sum14UL();
        t = t * 3UL + sum15UL();
        t = t * 3UL + sum28UL();
        t = t * 3UL + sum30UL();
        t = t * 3UL + sum31UL();
        t = t * 3UL + sum62UL();
        t = t * 3UL + sum63UL();
    }
    return (long)t;
}

int main(void)
{
    report(bench_x_mul_constants(1));
    return 0;
}
