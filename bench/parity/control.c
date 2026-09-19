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

long parity_control_demo(void)
{
    long first;

    demo_value = 7;
    demo_limit = 6;
    first = parity_control(&demo_value, &demo_limit);
    demo_value = -3;
    demo_limit = 5;
    return first * 1000L + parity_control(&demo_value, &demo_limit);
}
