// flags: -O0 | -O2 | -Os
// `x op= y` computes in the usual arithmetic conversions of x and y, then converts back (gcc.c-torture 20030128-1: `unsigned char x /=
// short y` with y = -5 was x / (unsigned char)y = 0, not (unsigned char)(50 / -5) = 246): the front end types the operation by the
// target's.
extern void report(long value);

unsigned char uc = 50;
volatile short sy = -5;
volatile unsigned char uy = 7;
volatile long ly = -3;
unsigned short us = 40000;
signed char sc = -100;
volatile int shift = 3;

int main(void)
{
    long check = 0;

    uc /= sy;               /* 246 */
    check = check * 7 + uc;
    uc = 200;
    uc %= sy;               /* 200 % -5 = 0 */
    check = check * 7 + uc;
    us /= ly;               /* (unsigned short)(40000 / -3) = 52429 */
    check = check * 7 + us;
    sc /= uy;               /* -100 / 7 = -14 */
    check = check * 7 + sc;
    sc = -100;
    sc >>= shift;           /* -13 */
    check = check * 7 + sc;
    uc = 0x81;
    uc <<= shift;           /* (unsigned char)(0x81 << 3) = 8 */
    check = check * 7 + uc;
    report(check);
    return 0;
}
