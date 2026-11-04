// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

long parity_memory(short *value, short *delta)
{
    *value = (short)(*value * 3 + *delta);
    return (long)*value * (long)*value;
}

static short demo_value;
static short demo_delta;

/* Read through volatile so no build folds the kernel to a constant. */
static volatile short input_value1 = 7;
static volatile short input_delta1 = -2;
static volatile short input_value2 = -4;
static volatile short input_delta2 = 11;

long bench_memory(void)
{
    long first;

    demo_value = input_value1;
    demo_delta = input_delta1;
    first = parity_memory(&demo_value, &demo_delta);
    demo_value = input_value2;
    demo_delta = input_delta2;
    return first * 1000L + parity_memory(&demo_value, &demo_delta);
}

int main(void)
{
    report(bench_memory());
    return 0;
}
