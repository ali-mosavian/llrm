/* 10000 pairs of longs (80000 bytes) in __huge memory: 8-byte elements. */
extern void report(long value);

struct pair {
    long x;
    long y;
};

struct pair __huge p[10000];

long bench_pairs(void)
{
    long t = 0;
    short i;

    for (i = 0; i < 10000; ++i) {
        p[i].x = i;
        p[i].y = i * 3L;
    }
    for (i = 0; i < 10000; ++i)
        t += p[i].y - p[i].x;
    return t;
}

int main(void)
{
    report(bench_pairs());
    return 0;
}
