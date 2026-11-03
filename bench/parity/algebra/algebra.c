// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Parity kernel; parameters by pointer as in the original. */
extern void report(long value);

long parity_algebra(short *a, short *b)
{
    long value;

    value = (long)*a * 9 + (long)*b * 5;
    return value * 3 - (long)*a;
}

static short demo_a;
static short demo_b;

/* Read through volatile so no build folds the kernel to a constant. */
static volatile short input_a1 = 23;
static volatile short input_b1 = 7;
static volatile short input_a2 = -11;
static volatile short input_b2 = 4;

long bench_algebra(void)
{
    long first;

    demo_a = input_a1;
    demo_b = input_b1;
    first = parity_algebra(&demo_a, &demo_b);
    demo_a = input_a2;
    demo_b = input_b2;
    return first * 1000L + parity_algebra(&demo_a, &demo_b);
}

int main(void)
{
    report(bench_algebra());
    return 0;
}
