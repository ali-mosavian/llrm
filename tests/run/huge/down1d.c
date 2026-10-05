/* 30000 longs (120000 bytes) in __huge memory, summed from the top down. */
extern void report(long value);

long __huge a[30000];

long bench_down1d(void)
{
    long t = 0;
    short i;

    for (i = 0; i < 30000; ++i)
        a[i] = i;
    for (i = 29999; i >= 0; --i)
        t = (t * 3 + a[i]) & 0xFFFFL;
    return t;
}

int main(void)
{
    report(bench_down1d());
    return 0;
}
