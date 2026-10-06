// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Parity kernel; parameters by pointer as in the original. */
extern void report(long value);

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

long bench_loop(void)
{
    long first;

    demo_count = 7;
    demo_seed = 5;
    first = parity_loop(&demo_count, &demo_seed);
    demo_count = 4;
    demo_seed = -3;
    return first * 1000L + parity_loop(&demo_count, &demo_seed);
}

int main(void)
{
    report(bench_loop());
    return 0;
}
