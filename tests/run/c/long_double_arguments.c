// flags: -O0 | -O2 | -Os | -O0 -m32 | -O2 -m32
// long double arguments between ints, and a long double result: at -m32 the front end made long double a double, then the x87 type
// was pushed as 10 bytes where the callee reads whole 12-byte slots, so the second argument and the ints after it were misread.
extern void report(long value);

long double mix(int a, long double x, int b, long double y, int c)
{
    return x * y + a - b + c;
}

int main(void)
{
    volatile long double x = 1.5L, y = 4.0L;

    report((long)mix(7, x, 2, y, 100));
    report((long)(mix(1, x, 1, y, 0) * 10));
    return 0;
}
