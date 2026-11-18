// flags: -fno-inline-functions
// An unsigned quotient and remainder of the same operands (one `__divmodhi4`): both are read back.
extern void report(long value);

static unsigned short xs[] = {61000, 4829, 40000, 65535, 30000, 1, 0, 32768, 50000, 65535, 12345};
static unsigned short ys[] = {37, 37, 300, 255, 7, 3, 5, 100, 1000, 65535, 2};

static void pair(unsigned short x, unsigned short y)
{
    unsigned short q = x / y, r = x % y;
    report(q);
    report(r);
}

int main(void)
{
    int i;
    for (i = 0; i < 11; ++i) pair(*(volatile unsigned short *)&xs[i], *(volatile unsigned short *)&ys[i]);
    return 0;
}
