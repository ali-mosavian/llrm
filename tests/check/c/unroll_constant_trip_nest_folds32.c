// RUN: llrm-cc %s -m32 -mabi=sysv -O3 -march=i486 -S -o /dev/stdout
// A nest of 7 x 6 constant trips over constant tables folds to its checksum once both loops unroll. The inner loop alone priced 1747 B for 192 clk and was vetoed, so the divisions stayed (tests/run int64_division, UP1: 14 -> 198 instructions, 8 calls).
// CHECK-LABEL: _nest proc
// CHECK-NOT: call
// CHECK: _nest endp
typedef long long s64;
typedef unsigned long long u64;
static s64 quotient(s64 a, s64 b) { return a / b; }
static s64 modulo(s64 a, s64 b) { return a % b; }
static u64 uquotient(u64 a, u64 b) { return a / b; }
static u64 umodulo(u64 a, u64 b) { return a % b; }
static unsigned long mix(unsigned long acc, u64 r) { return acc * 31UL + (unsigned long)r + (unsigned long)(r >> 32) * 7UL; }
unsigned long nest(void)
{
    static const s64 numerators[] = { 1LL, -1LL, 100000000000LL, -100000000000LL, 0x123456789ABCDEFLL, 7LL, -7LL };
    static const s64 divisors[] = { 2LL, -3LL, 1000LL, 4294967296LL, -4294967297LL, 123456789012LL };
    unsigned long c0 = 0, c1 = 0, c2 = 0, c3 = 0;
    int i, j;
    for (i = 0; i < 7; ++i)
        for (j = 0; j < 6; ++j) {
            c0 = mix(c0, (u64)quotient(numerators[i], divisors[j]));
            c1 = mix(c1, (u64)modulo(numerators[i], divisors[j]));
            c2 = mix(c2, uquotient((u64)numerators[i], (u64)divisors[j] & 0xFFFFFFFFFFULL));
            c3 = mix(c3, umodulo((u64)numerators[i], ((u64)divisors[j] & 0xFFFFFFFFFFULL) | 1ULL));
        }
    return c0 ^ c1 ^ c2 ^ c3;
}
