long parity_branch(short *value)
{
    if (*value < 0)
        return (long)*value * 7 + 3;
    return (long)*value * 5 - 9;
}

static short demo_value;

long parity_branch_demo(void)
{
    long first;

    demo_value = -13;
    first = parity_branch(&demo_value);
    demo_value = 21;
    return first * 1000L + parity_branch(&demo_value);
}
