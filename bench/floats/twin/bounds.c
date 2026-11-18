// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Twin of floats.nib with its semantics: the loop has no array and no local needing a zero, so it is floats.c as it is. */
extern void report(long value);

long bench_floats(unsigned short iterations)
{
    volatile double value = 1.0;
    unsigned short i;

    for (i = 0; i < iterations; ++i) value = (value * 1.0009765625 + 0.125) / 1.00048828125;
    return (long)(value * 1000.0);
}

int main(void)
{
    report(bench_floats(1000));
    return 0;
}
