/* 40000 words (80000 bytes) in __huge memory filled with one value no one byte repeats: past 64K. */
extern void report(long value);

short __huge a[40000];

long bench_fillw(void)
{
    long t = 0;
    unsigned short i;

    for (i = 0; i < 40000; ++i)
        a[i] = 4660;
    for (i = 0; i < 40000; ++i)
        t += a[i];
    return t;
}

int main(void)
{
    report(bench_fillw());
    return 0;
}
