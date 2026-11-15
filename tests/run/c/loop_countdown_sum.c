// flags: -O2 | -O3
// targets: !ptr16
// A count-down loop that sums a value of its induction (not a closed form: scalar evolution would fold `i * 3`), from a run-time start (n down to 0, and 1999 down to 0). The vector loop's
// lanes count down from the start, `<s, s-1, ..>`; a lane order or step the wrong way sums other values. The oracle is a volatile
// loop, which is never vectorized. Prints 0, else the bad count then the first failing n.
extern void report(long value);

__attribute__((noinline)) unsigned down(int n)
{
    unsigned s = 0;
    for (int i = n; i >= 0; i--) s += (((unsigned)i * 3u) ^ ((unsigned)i >> 2));
    return s;
}

__attribute__((noinline)) unsigned down_fixed(void)
{
    unsigned s = 0;
    for (int i = 1999; i >= 0; i--) s += (((unsigned)i * 3u) ^ ((unsigned)i >> 2)) + 1u;
    return s;
}

__attribute__((noinline)) unsigned down_oracle(int n)
{
    volatile unsigned s = 0;
    for (int i = n; i >= 0; i--) s = s + (((unsigned)i * 3u) ^ ((unsigned)i >> 2));
    return s;
}

__attribute__((noinline)) unsigned down_fixed_oracle(void)
{
    volatile unsigned s = 0;
    for (int i = 1999; i >= 0; i--) s = s + (((unsigned)i * 3u) ^ ((unsigned)i >> 2)) + 1u;
    return s;
}

int main(void)
{
    long bad = 0, first = -1;
    for (int n = 0; n < 300; n++)
        if (down(n) != down_oracle(n)) {
            if (!bad) first = n;
            bad++;
        }
    if (down_fixed() != down_fixed_oracle()) {
        if (!bad) first = 2000;
        bad++;
    }
    report(bad);
    if (bad) report(first);
    return 0;
}
