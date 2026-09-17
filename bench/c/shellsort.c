unsigned long bench_shellsort(unsigned short seed)
{
    unsigned short values[64];
    unsigned short i, gap;
    unsigned long checksum = 0;

    for (i = 0; i < 64; ++i) values[i] = (unsigned short)(((unsigned long)i * 109 + 37) ^ (i << 7) ^ seed);
    for (gap = 32; gap; gap >>= 1)
        for (i = gap; i < 64; ++i) {
            unsigned short at = i, value = values[i];
            while (at >= gap && values[at - gap] > value) {
                values[at] = values[at - gap];
                at = (unsigned short)(at - gap);
            }
            values[at] = value;
        }
    for (i = 0; i < 64; ++i) checksum += (unsigned long)values[i] * (i + 1);
    return checksum;
}
