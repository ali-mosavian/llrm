// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
extern void report(long value);

long parity_memory(short *value, short *delta)
{
    *value = (short)(*value * 3 + *delta);
    return (long)*value * (long)*value;
}

static short demo_value;
static short demo_delta;

long bench_memory(void)
{
    long first;

    demo_value = 7;
    demo_delta = -2;
    first = parity_memory(&demo_value, &demo_delta);
    demo_value = -4;
    demo_delta = 11;
    return first * 1000L + parity_memory(&demo_value, &demo_delta);
}

int main(void)
{
    report(bench_memory());
    return 0;
}
