// flags: -O0 | -O2 | -Os
// An i64 shifted by a count the program computes, every count 0 to 63 (gcc.c-torture 920501-2, longlong: "an i64 shifted by a
// variable"); at -O0 a constant count is `zext i32 30 to i64`, which was refused too. The checksums mix both halves.
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
    unsigned long left = 0, logical = 0, arith = 0, fixed = 0;
    int i;
    unsigned c;
    for (i = 0; i < 4; ++i)
        for (c = 0; c < 64; ++c) {
            u64 v = values[i];
            left = mix(left, v << c);
            logical = mix(logical, v >> c);
            arith = mix(arith, (u64)((s64)v >> c));
        }
    for (i = 0; i < 4; ++i)
        fixed = mix(fixed, (values[i] >> 30) + (values[i] << 33) + (u64)((s64)values[i] >> 35));
    report((long)left);
    report((long)logical);
    report((long)arith);
    report((long)fixed);
    return 0;
}
