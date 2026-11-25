// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* A gapped insertion sort over a local word array: a byte index beside a word pointer reach one address use, and the formula
   was summed in the byte's type (a wrong sort at -O1, a halt at -O2 and -Os on the 6502). */
extern void report(long value);

static unsigned long __attribute__((noinline)) sorted_checksum(unsigned short seed)
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

int main(void)
{
    report((long)sorted_checksum(0));
    report((long)sorted_checksum(0x5a5a));
    return 0;
}
