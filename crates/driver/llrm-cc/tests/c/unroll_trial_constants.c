extern void report(long value);

typedef unsigned long long u64;

__attribute__((noinline)) static unsigned long mix(unsigned long acc, u64 r)
{
    return acc * 31UL + (unsigned long)r + (unsigned long)(r >> 32) * 7UL;
}

static const u64 values[2] = { 0x0123456789ABCDEFULL, 0xFFFFFFFFFFFFFFFFULL };

int main(void)
{
    unsigned long left = 0;
    int i;
    unsigned c;
    for (i = 0; i < 2; ++i)
        for (c = 0; c < 64; ++c) {
            u64 v = values[i];
            u64 n = c;
            left = mix(left, v << (n & 31));
        }
    report((long)left);
    return 0;
}
