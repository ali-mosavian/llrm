// flags: -O2
// bar: not comparable (SDCC double is 32 bits: it computes a different program)
// 3.0 / 1.0 ran as 4 on the z80: the zero test of the quotient's integer bit was dropped across an `add hl` and the
// `jr nz` after the divide read the add's flags. It needs a mantissa quotient of at least 1 with fraction bits.
extern void report(long value);

int main(void)
{
    volatile double a = 3.0, b = 1.0, c = 9.0, d = 3.0, e = 1.0, f = 3.0;

    report((long)(a / b));
    report((long)(c / d));
    report((long)(e / f * 1000.0));
    report((long)(a / 2.0 * 10.0));
    return 0;
}
