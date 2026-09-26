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

/* Exercise every sign path with a divisor that does not fit one dword. */
i64 signedWide64(i64 left, i64 right)
{
    i64 quotient = left / right;
    i64 remainder = left % right;
    return quotient * 3 + remainder;
}

/* 0 means that every known-answer test passed. */
int euclid64Check(void)
{
    return gcd64(0xfedcba9876543210ULL, 0x1234567890abcdefULL) != 15
        || signedCrunch64(-123456789012345LL, 19) != -206577158321LL
        || signedWide64(0x7123456789abcdefLL, 0x100000003LL) != 0x189abcdefLL
        || signedWide64(0x7123456789abcdefLL, -0x100000003LL) != -4784116341LL
        || signedWide64(-0x7123456789abcdefLL, -0x100000003LL) != 0x11d27d275LL
        || signedWide64(-0x7123456789abcdefLL, 0x100000003LL) != -6604705263LL;
}

int main(void)
{
    return euclid64Check();
}
