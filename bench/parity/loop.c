long parity_loop(short *count, short *seed)
{
    long total;
    short index;

    total = *seed;
    index = 0;
    while (index < *count) {
        total += (long)index * *seed + 3;
        ++index;
    }
    return total;
}

static short demo_count;
static short demo_seed;

long parity_loop_demo(void)
{
    long first;

    demo_count = 7;
    demo_seed = 5;
    first = parity_loop(&demo_count, &demo_seed);
    demo_count = 4;
    demo_seed = -3;
    return first * 1000L + parity_loop(&demo_count, &demo_seed);
}
