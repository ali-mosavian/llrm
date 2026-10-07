// flags: -O0 | -O2 | -Os
// 64-bit division and remainder by a value the program computes (gcc.c-torture 920604-1 at -O0: the program exited with a
// fault): every combination of sign, with divisors of one and of two words.
extern void report(long value);

typedef long long s64;
typedef unsigned long long u64;

static s64 quotient(s64 a, s64 b) { return a / b; }
static s64 modulo(s64 a, s64 b) { return a % b; }
static u64 uquotient(u64 a, u64 b) { return a / b; }
static u64 umodulo(u64 a, u64 b) { return a % b; }

int main(void)
{
    static const s64 numerators[] = { 1LL, -1LL, 100000000000LL, -100000000000LL, 0x123456789ABCDEFLL, 7LL, -7LL };
    static const s64 divisors[] = { 2LL, -3LL, 1000LL, 4294967296LL, -4294967297LL, 123456789012LL };
    unsigned long checks[4] = { 0, 0, 0, 0 };
    int i, j;
    for (i = 0; i < 7; ++i)
        for (j = 0; j < 6; ++j) {
            s64 q = quotient(numerators[i], divisors[j]);
            s64 m = modulo(numerators[i], divisors[j]);
            u64 uq = uquotient((u64)numerators[i], (u64)divisors[j] & 0xFFFFFFFFFFULL);
            u64 um = umodulo((u64)numerators[i], ((u64)divisors[j] & 0xFFFFFFFFFFULL) | 1ULL);
            checks[0] = checks[0] * 31UL + (unsigned long)q + (unsigned long)(q >> 32) * 7UL;
            checks[1] = checks[1] * 31UL + (unsigned long)m + (unsigned long)(m >> 32) * 7UL;
            checks[2] = checks[2] * 31UL + (unsigned long)uq + (unsigned long)(uq >> 32) * 7UL;
            checks[3] = checks[3] * 31UL + (unsigned long)um + (unsigned long)(um >> 32) * 7UL;
        }
    for (i = 0; i < 4; ++i)
        report((long)checks[i]);
    return 0;
}
