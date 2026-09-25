typedef unsigned long long u64;

/* SplitMix64's avalanche step: shifts, xor and wrapping multiplication. */
u64 mix64(u64 value)
{
    value ^= value >> 30;
    value *= 0xbf58476d1ce4e5b9ULL;
    value ^= value >> 27;
    value *= 0x94d049bb133111ebULL;
    return value ^ (value >> 31);
}

/* 0 means that the known-answer test passed. */
int mix64Check(void)
{
    return mix64(0x123456789abcdef0ULL) != 0x9629f58e8ec5b906ULL;
}

int main(void)
{
    return mix64Check();
}
