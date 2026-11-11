// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Histogram of a 4096-byte table; result is sum(counts[i] * (i + 1)). */
extern void report(long value);

static unsigned char data[4096];
static short counts[256];

static void histogram(short *seed)
{
    short index;

    for (index = 0; index < 4096; ++index)
        ++counts[(data[index] * 7 + *seed) & 255];
}

long bench_histo(short seed)
{
    short i;
    long sum = 0;

    for (i = 0; i < 4096; ++i)
        data[i] = (i & 127) * (i >> 5);
    for (i = 0; i < 256; ++i)
        counts[i] = 0;
    histogram(&seed);
    for (i = 0; i < 256; ++i)
        sum += (long)counts[i] * (i + 1);
    return sum;
}

int main(void)
{
    report(bench_histo(3));
    return 0;
}
