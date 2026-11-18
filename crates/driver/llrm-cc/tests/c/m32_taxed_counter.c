long sum_ring(void)
{
    long buf[1024];
    long total = 0;
    short i;

    for (i = 0; i < 1024; ++i)
        buf[i] = (long)(i + 1) * 3 - 7;
    for (i = 0; i < 6000; ++i)
        total += buf[(i * 5 + 3) & 1023];
    return total;
}
