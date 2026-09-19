long parity_algebra(short *a, short *b)
{
    long value;

    value = (long)*a * 9 + (long)*b * 5;
    return value * 3 - (long)*a;
}

static short demo_a;
static short demo_b;

long parity_algebra_demo(void)
{
    long first;

    demo_a = 23;
    demo_b = 7;
    first = parity_algebra(&demo_a, &demo_b);
    demo_a = -11;
    demo_b = 4;
    return first * 1000L + parity_algebra(&demo_a, &demo_b);
}
