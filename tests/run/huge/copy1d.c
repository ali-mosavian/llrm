/* Two arrays of 20000 longs (80000 bytes each) in __huge memory: one read into the other. */
extern void report(long value);

long __huge a[20000];
long __huge b[20000];

long bench_copy1d(void)
{
    long t = 0;
    short i;

    for (i = 0; i < 20000; ++i)
        a[i] = i;
    for (i = 0; i < 20000; ++i)
        b[i] = a[i] * 2 + 1;
    for (i = 0; i < 20000; ++i)
        t += b[i];
    return t;
}

int main(void)
{
    report(bench_copy1d());
    return 0;
}
