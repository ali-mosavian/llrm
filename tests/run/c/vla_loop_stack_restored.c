// flags: -O1 | -O2 | -O3 | -Os
// targets: !dos !z80-sdcc !6502-sim65 !65c02-sim65
/* arm64 -Os: a stacksave read in one block and restored in another was read again after the alloca lowered sp, so the stackrestore freed nothing and each trip's array sat lower than the last (reports 1 from the fourth trip). */
extern void report(long value);

static int vla(int n)
{
    char a[n];
    int i;

    for (i = 0; i < n; ++i)
        a[i] = (char)i;
    return a[n - 1];
}

__attribute__((noinline)) static long nested(void)
{
    long addr = 0, first = 0;
    int i, j;

    for (i = 1; i < 7; ++i) {
        char a[i * 3];

        for (j = 0; j < i * 3; ++j)
            a[j] = (char)(i + j);
        if (!first)
            first = (long)(unsigned long)a;
        report(first - (long)(unsigned long)a > 32);
        addr += a[i * 3 - 1] + vla(i);
    }
    return addr;
}

int main(void)
{
    report(nested());
    return 0;
}
