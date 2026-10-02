extern void report(long value);

long bench_scalar(void)
{
    long total;
    short index;

    total = 17;
    for (index = 0; index < 8; ++index)
        total += (long)(index * 3 + 1) * (29 - index * 2);
    return total;
}

int main(void)
{
    report(bench_scalar());
    return 0;
}
