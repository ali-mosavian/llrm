// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);
extern int keep(int value); /* opaque: the compiler cannot fold the kernel away */

long bench_scalar(short seed)
{
    long total;
    short index;

    total = 17;
    for (index = 0; index < 8; ++index)
        total += (long)(index * 3 + 1 + seed) * (29 - index * 2);
    return total;
}

int main(void)
{
    report(bench_scalar(keep(0)));
    return 0;
}
