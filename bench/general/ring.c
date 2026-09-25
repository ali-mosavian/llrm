static long buf[1024];

long ring_sum(short *count)
{
    long total;
    short index;

    total = 0;
    for (index = 0; index < *count; ++index)
        total += buf[(index * 5 + 3) & 1023];
    return total;
}
