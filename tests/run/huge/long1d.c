/* 30000 longs (120000 bytes) in __huge memory: fill, then sum. */
extern void report(long value);

long __huge a[30000];

long bench_long1d(void)
{
    long t = 0;
    short i;

    for (i = 0; i < 30000; ++i)
        a[i] = i + 5L;
    for (i = 0; i < 30000; ++i)
        t += a[i];
    return t;
}

int main(void)
{
    report(bench_long1d());
    return 0;
}
