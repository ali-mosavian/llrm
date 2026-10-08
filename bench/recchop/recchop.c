// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

static int values[64];

/* Two calls, the second the last thing done, and a loop between the entry and the calls. */
static long chop(int lo, int hi)
{
    long sum = 0;
    int i, mid;

    if (hi - lo < 2) return 0;
    mid = (lo + hi) / 2;
    for (i = lo; i < hi; ++i) sum += values[i] & mid;
    return sum + chop(lo, mid) + chop(mid, hi);
}

long bench_recchop(int n)
{
    int i;

    for (i = 0; i < 64; ++i) values[i] = (i * 37 + n) & 127;
    return chop(0, 64);
}

int main(void)
{
    report(bench_recchop(5));
    return 0;
}
