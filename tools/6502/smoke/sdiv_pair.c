// flags: -fno-inline-functions
// A signed quotient and remainder of the same operands (one `__divmodhi4`): both are read back.
extern void report(long value);

static short xs[] = {-4829, 4829, -4829, 4829, 30000, -30000, 1, -1, 0, 32767, -32767};
static short ys[] = {37, 37, -37, -37, 7, 255, 3, 5, 9, 100, -1000};

static void pair(short x, short y)
{
    short q = x / y, r = x % y;
    report(q);
    report(r);
}

int main(void)
{
    int i;
    for (i = 0; i < 11; ++i) pair(*(volatile short *)&xs[i], *(volatile short *)&ys[i]);
    return 0;
}
