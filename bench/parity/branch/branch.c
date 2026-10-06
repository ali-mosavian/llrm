// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Parity kernel; parameters by pointer as in the original. */
extern void report(long value);

long parity_branch(short *value)
{
    if (*value < 0)
        return (long)*value * 7 + 3;
    return (long)*value * 5 - 9;
}

static short demo_value;

long bench_branch(void)
{
    long first;

    demo_value = -13;
    first = parity_branch(&demo_value);
    demo_value = 21;
    return first * 1000L + parity_branch(&demo_value);
}

int main(void)
{
    report(bench_branch());
    return 0;
}
