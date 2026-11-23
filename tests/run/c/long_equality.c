// flags: -O1 | -O2 | -Os
// bar: none (new program)
// == and != of two longs that differ in one byte only (CMPZ32), against zero, a constant and a variable, as a
// branch and as a value, with the bytes in registers and in memory.
extern void report(long value);

volatile unsigned long a[6] = {0UL, 0x000000FFUL, 0x0000FF00UL, 0x00FF0000UL, 0xFF000000UL, 0x01020304UL};
volatile unsigned long b[6] = {0UL, 0x000000FEUL, 0x0000FE00UL, 0x00FE0000UL, 0xFE000000UL, 0x01020304UL};

static int is_zero(unsigned long x) { return x == 0; }
static int differ(unsigned long x, unsigned long y) { return x != y; }
static int is_const(unsigned long x) { return x == 0x01020304UL; }

int main(void)
{
    int i;
    unsigned long n;

    for (i = 0; i < 6; ++i) {
        unsigned long x = a[i], y = b[i];

        report(is_zero(x));
        report(differ(x, y));
        report(is_const(x));
        if (x == y)
            report(100 + i);
        else
            report(200 + i);
    }
    for (n = 0x102UL; n != 0; --n)
        report((long)n);
    for (n = 0x10003UL; n != 0xFFFEUL; --n)
        report((long)n);
    return 0;
}
