// flags: -O0 | -O2 | -Os
// An i64 shifted by a count masked to 5 bits: the compiler drops the fix-up for counts from 32 (known bits), so every
// count 0 to 31 is checked against the shift the program means, both halves mixed.
extern void report(long value);

typedef unsigned long long u64;
typedef long long s64;

static unsigned long mix(unsigned long acc, u64 r)
{
    return acc * 31UL + (unsigned long)r + (unsigned long)(r >> 32) * 7UL;
}

static const u64 values[4] = { 0x0123456789ABCDEFULL, 0xFFFFFFFFFFFFFFFFULL, 0x8000000000000001ULL, 0x00000000FFFF0001ULL };

int main(void)
{
    unsigned long left = 0, logical = 0, arith = 0;
    int i;
    unsigned c;
    for (i = 0; i < 4; ++i)
        for (c = 0; c < 64; ++c) {
            u64 v = values[i];
            u64 n = c;
            left = mix(left, v << (n & 31));
            logical = mix(logical, v >> (n & 31));
            arith = mix(arith, (u64)((s64)v >> (n & 31)));
        }
    report((long)left);
    report((long)logical);
    report((long)arith);
    return 0;
}
