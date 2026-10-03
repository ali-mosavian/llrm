/* 21843 triples of shorts (131058 bytes) in __huge memory: 6-byte elements, a stride that does not divide 64K, over three windows. */
extern void report(long value);

struct tri {
    short a;
    short b;
    short c;
};

struct tri __huge p[21843];

long bench_tri(void)
{
    long t = 0;
    short i;

    for (i = 0; i < 21843; ++i) {
        p[i].a = i;
        p[i].c = i & 255;
    }
    for (i = 0; i < 21843; ++i)
        t += p[i].c - p[i].a;
    return t;
}

int main(void)
{
    report(bench_tri());
    return 0;
}
