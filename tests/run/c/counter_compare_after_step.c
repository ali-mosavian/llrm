// flags: -O2 -fno-inline-functions | -Os -fno-inline-functions
/* The 12-trip loop's counter narrows to a byte kept in `a`; the exit tests the old value, `i < 11`, and steps it
   with `clc; adc #1`. The 6502 compare was moved above the step, `bcs` read the step's carry, and the loop ran 256
   trips: 239 wrong quotients (div_constants_short at -Os). Prints the count of wrong quotients or remainders (0). */
extern void report(long value);
static long c(void)
{
    volatile short dv = 3;
    short x, q, r;
    long bad = 0, i;
    for (i = 0; i < 12; ++i) {
        short hi = 16383;
        x = (short)(i < 2 ? i : i < 4 ? hi - (i - 2) : i < 6 ? (short)3 + (i - 5) : i < 8 ? (short)(hi / 3 * 3) - (i - 6) : i < 10 ? (short)(hi - 3 + (i - 8)) : (short)(2 * 3 - (i - 10)));
        q = x / 3; r = x % 3;
        if (q != x / dv || r != x % dv) ++bad;
    }
    return bad;
}
int main(void) { report(c()); return 0; }
