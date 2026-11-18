// flags: -O0 | -O2 | -Os | -O0 -m32 | -O2 -m32
// The twin of long_double_arguments with float for double: SDCC's double is 32 bits ("types double, long double not supported. Assuming float"), so this is the program its columns run.
// Float literals carry `f` as well: SDCC reads every `double`, constant or type, as a float.
// float arguments between ints, and a float result: at -m32 the front end made float a float, then the x87 type
// was pushed as 10 bytes where the callee reads whole 12-byte slots, so the second argument and the ints after it were misread.
extern void report(long value);

float mix(int a, float x, int b, float y, int c)
{
    return x * y + a - b + c;
}

int main(void)
{
    volatile float x = 1.5L, y = 4.0L;

    report((long)mix(7, x, 2, y, 100));
    report((long)(mix(1, x, 1, y, 0) * 10));
    return 0;
}
