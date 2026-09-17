/* Return prime count and sum below 1024 in one independently checkable word. */
unsigned long bench_sieve(unsigned short limit)
{
    unsigned char composite[1024];
    unsigned short i, multiple, count = 0;
    unsigned long sum = 0;

    for (i = 0; i < limit; ++i) composite[i] = 0;
    for (i = 2; i < limit; ++i) {
        if (composite[i]) continue;
        ++count;
        sum += i;
        if (i <= 31)
            for (multiple = (unsigned short)(i * i); multiple < limit; multiple = (unsigned short)(multiple + i))
                composite[multiple] = 1;
    }
    return ((unsigned long)count << 16) ^ sum;
}
