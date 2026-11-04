// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Parity kernel; parameters by pointer as in the original. */
extern void report(long value);

long parity_control(short *value, short *limit)
{
    long total;
    short index;

    total = 0;
    for (index = 0; index < *limit; ++index) {
        if ((index & 1) == 0)
            total += (long)*value + index;
        else
            total -= (long)*value - index;
    }
    return total;
}

static short demo_value;
static short demo_limit;

/* Read through volatile so no build folds the kernel to a constant. */
static volatile short input_value1 = 7;
static volatile short input_limit1 = 6;
static volatile short input_value2 = -3;
static volatile short input_limit2 = 5;

long bench_control(void)
{
    long first;

    demo_value = input_value1;
    demo_limit = input_limit1;
    first = parity_control(&demo_value, &demo_limit);
    demo_value = input_value2;
    demo_limit = input_limit2;
    return first * 1000L + parity_control(&demo_value, &demo_limit);
}

int main(void)
{
    report(bench_control());
    return 0;
}
