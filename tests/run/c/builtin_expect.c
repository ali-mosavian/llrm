// flags: -O0 | -O2 | -Os
// __builtin_expect(x, c) in valid C compiled to a call of an undefined extern and the program did not link.
extern void report(long value);

static int pick(int x)
{
    if (__builtin_expect(x, 0))
        return 1;
    return 2;
}

int main(void)
{
    report(__builtin_expect(7, 1));
    report(pick(0));
    report(pick(5));
    return 0;
}
