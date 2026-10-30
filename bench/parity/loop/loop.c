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

/* Read through volatile so no build folds the kernel to a constant. */
static volatile short input_count_a = 7;
static volatile short input_seed_a = 5;
static volatile short input_count_b = 4;
static volatile short input_seed_b = -3;

long bench_loop(void)
{
    long first;

    demo_count = input_count_a;
    demo_seed = input_seed_a;
    first = parity_loop(&demo_count, &demo_seed);
    demo_count = input_count_b;
    demo_seed = input_seed_b;
    return first * 1000L + parity_loop(&demo_count, &demo_seed);
}

int main(void)
{
    report(bench_loop());
    return 0;
}
