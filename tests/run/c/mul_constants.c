// flags: -O2 -fno-inline-functions | -Os -fno-inline-functions | -O2 -fno-inline-functions -m32 | -Os -fno-inline-functions -m32 | -O2 -fno-inline-functions -march=pentium -m32
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

int main(void)
{
    report((long)sum3UL());
    report((long)sum5UL());
    report((long)sum9UL());
    report((long)sum6UL());
    report((long)sum10UL());
    report((long)sum12UL());
    report((long)sum17UL());
    report((long)sum18UL());
    report((long)sum20UL());
    report((long)sum24UL());
    report((long)sum25UL());
    report((long)sum36UL());
    report((long)sum40UL());
    report((long)sum45UL());
    report((long)sum72UL());
    report((long)sum81UL());
    report((long)sum100UL());
    report((long)sum125UL());
    report((long)sum127UL());
    report((long)sum129UL());
    report((long)sum255UL());
    report((long)sum257UL());
    report((long)sum1000UL());
    report((long)sum65537UL());
    report((long)sum7UL());
    report((long)sum14UL());
    report((long)sum15UL());
    report((long)sum28UL());
    report((long)sum30UL());
    report((long)sum31UL());
    report((long)sum62UL());
    report((long)sum63UL());
    return 0;
}
