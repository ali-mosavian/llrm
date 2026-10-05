extern void report(long value);
extern int keep(int value); /* opaque: the compiler cannot fold the kernel away */

typedef struct {
    short x;
    short y;
} Pair;

long bench_parity(short seed)
{
    Pair points[8];
    long total;
    short index;

    total = 17;
    for (index = 0; index < 8; ++index) {
        points[index].x = index * 3 + 1 + seed;
        points[index].y = 29 - index * 2;
    }
    for (index = 0; index < 8; ++index)
        total += (long)points[index].x * points[index].y;
    return total;
}

int main(void)
{
    report(bench_parity(keep(0)));
    return 0;
}
