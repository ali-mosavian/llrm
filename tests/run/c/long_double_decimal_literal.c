// flags: -O0 -mlong-double-80 | -O2 -mlong-double-80 | -Os -mlong-double-80
// 0.1L and 1.1L are not the doubles 0.1 and 1.1: the pin writes them in the IR as the decimals 1.000000e-01 and 1.100000e+00, and the
// reader rounded those to a double, so the low 11 bits of the significand were zero (0xD000 for the lowest word, not 0xCCCD).
extern void report(long value);

static unsigned lowest_word(volatile long double *x)
{
    return *(volatile unsigned short *)x;
}

int main(void)
{
    volatile long double tenth = 0.1L;
    volatile long double over = 1.1L;
    report(lowest_word(&tenth));
    report(lowest_word(&over));
    return 0;
}
