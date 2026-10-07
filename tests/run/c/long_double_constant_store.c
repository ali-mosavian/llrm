// flags: -O0 | -O2 | -Os
// A local `long double` initialized from a constant (gcc.c-torture 20020413-1 on the 16-bit target, where it is the x87's 10 bytes):
// only its first dword was stored, the other six bytes were what the stack held, and the program looped on a compare of garbage.
extern void report(long value);

static void dirty(void)
{
    volatile unsigned char junk[96];
    int i;
    for (i = 0; i < 96; ++i)
        junk[i] = 0xFF;
}

static long show(void)
{
    long double a = 1.0l;
    long double b = 3.5l;
    long double c = -2.25l;
    return (long)(a * 100.0l + b * 10.0l + c * 4.0l);
}

int main(void)
{
    dirty();
    report(show());
    dirty();
    report(show() + 1);
    return 0;
}
