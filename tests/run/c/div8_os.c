// flags: -Os -march=i486 -m32
/* Byte divides at -Os stay DIV8r/IDIV8r (`isIntDivCheap`): the quotient and remainder of every byte by a constant and by a variable,
   summed with position weights so a swapped or sign-extended wrong half cannot cancel (SD206 once kept a multiply, SD302 is the AH read). */
extern void report(long value);

long total(void)
{
    unsigned long sum = 0;
    int a, b;

    for (a = 0; a < 256; ++a) {
        unsigned char u = (unsigned char)a;
        signed char s = (signed char)a;
        sum = sum * 3 + u / 6 + (u % 6) * 7 + s / 6 + (s % 6) * 5 + u / 100 + (u % 100);
        sum = sum * 3 + s / -7 + (s % -7) * 3;
        for (b = 1; b < 256; b += 17) {
            unsigned char v = (unsigned char)b;
            signed char t = (signed char)b;
            sum = sum * 3 + u / v + (u % v) * 11 + s / t + (s % t) * 13;
        }
    }
    return (long)sum;
}

int main(void)
{
    report(total());
    return 0;
}
