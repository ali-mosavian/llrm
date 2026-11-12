// flags: -O2 | -Os
// A 64-bit divide and remainder by a divisor of 31 to 32 bits: the remainder shifted left one dividend bit at a time passes 32 bits
// (2^32 / 0xFFFFFFFF is 1, not 0). Unsigned and signed, quotient and remainder, as the low and high halves of each result.
extern void report(long value);
typedef long long s64;
typedef unsigned long long u64;

static const u64 A[] = { 1ULL << 32, (1ULL << 33) - 1, (1ULL << 63) - 1, 1ULL << 62, 0xFFFFFFFFFFFFFFFFULL, 0x123456789ABCDEF0ULL,
    (1ULL << 48) + 12345 };
static const unsigned long B[] = { 0xFFFFFFFFUL, 0x80000001UL, 0x80000000UL, 0xC0000000UL, 0xFFFFFFFEUL, 0x90000000UL, 0x80003039UL };

static void both(u64 v)
{
    report((long)(unsigned long)v);
    report((long)(unsigned long)(v >> 32));
}

int main(void)
{
    volatile u64 va, vb;
    int i, j;
    for (i = 0; i < (int)(sizeof A / sizeof A[0]); ++i)
        for (j = 0; j < (int)(sizeof B / sizeof B[0]); ++j) {
            va = A[i];
            vb = B[j];
            both(va / vb);
            both(va % vb);
            both((u64)((s64)va / (s64)vb));
            both((u64)((s64)va % (s64)vb));
        }
    return 0;
}
