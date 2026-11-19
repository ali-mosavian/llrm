// flags: -O0 | -O2 | -Os
// A compare whose flags are read after a call: the scheduler left the call between `subs` and the branch, which read the callee's flags.
extern void report(long value);

__attribute__((noinline)) static int callee(int x)
{
    return x < 5 ? x + 100 : x * 3;
}

__attribute__((noinline)) static int pick(int n)
{
    int r = callee(n - 12);
    if (n == 12)
        return r + 1;
    return r + 2;
}

int main(void)
{
    report(pick(12));
    report(pick(13));
    report(pick(30));
    return 0;
}
