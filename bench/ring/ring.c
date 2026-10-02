/* Fill and sum a 1024-entry long ring, in the kernel; Nib has no globals. */
extern void report(long value);

long bench_ring(void)
{
    long buf[1024];
    long total;
    short i;

    for (i = 0; i < 1024; ++i)
        buf[i] = (long)(i + 1) * 3 - 7;
    total = 0;
    for (i = 0; i < 6000; ++i)
        total += buf[(i * 5 + 3) & 1023];
    return total;
}

int main(void)
{
    report(bench_ring());
    return 0;
}
