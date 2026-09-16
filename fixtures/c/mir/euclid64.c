typedef signed long long i64;
typedef unsigned long long u64;

u64 gcd64(u64 left, u64 right)
{
    while (right != 0) {
        u64 remainder = left % right;
        left = right;
        right = remainder;
    }
    return left;
}

/* Exercise signed quotient/remainder rules without overflowing i64. */
i64 signedCrunch64(i64 value, unsigned rounds)
{
    unsigned i;
    for (i = 0; i < rounds; ++i) {
        i64 quotient = value / 7;
        i64 remainder = value % 7;
        value = quotient * 5 - remainder * 11 + (i64)(i + 1);
    }
    return value;
}

/* 0 means that both known-answer tests passed. */
int euclid64Check(void)
{
    return gcd64(0xfedcba9876543210ULL, 0x1234567890abcdefULL) != 15
        || signedCrunch64(-123456789012345LL, 19) != -206577158321LL;
}

int main(void)
{
    return euclid64Check();
}
