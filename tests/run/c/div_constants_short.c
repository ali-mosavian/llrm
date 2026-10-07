// flags: -O2 -fno-inline-functions | -Os -fno-inline-functions | -O2 -fno-inline-functions -m32 | -Os -fno-inline-functions -m32 | -O2 -fno-inline-functions -march=pentium -m32
// A division or remainder by a constant is a multiply by its reciprocal where the target prices it cheaper: the quotient and
// remainder of each constant divisor, over random and edge numerators of every length, equal those of the same division by
// a variable divisor (a real divide). Prints the count of numerators where they differ (0), then a checksum
// of the quotients and remainders so a program that folded both sides cannot pass.
extern void report(long value);

static unsigned long state = 2463534242UL;
static unsigned long next(void) { state ^= state << 13; state ^= state >> 17; state ^= state << 5; return state; }

static unsigned long sum;

#define CASE(T, NAME, D, SIGNED)\
static long NAME(void)\
{\
    volatile T dv = D;\
    T x, q, r;\
    long bad = 0, i;\
    for (i = 0; i < 1500; ++i) {\
        unsigned long w = next(), shape = next() & 7;\
        x = (T)(shape == 0 ? w : shape == 1 ? w >> 16 : shape == 2 ? w >> 24 : shape == 3 ? w >> 29 : w);\
        if (SIGNED && (next() & 1)) x = (T)-x;\
        q = x / D; r = x % D;\
        if (q != x / dv || r != x % dv) ++bad;\
        sum += (unsigned long)q * 31UL + (unsigned long)r;\
    }\
    for (i = 0; i < 12; ++i) {\
        T hi = (T)(((unsigned long)1 << (sizeof(T) * 8 - 1 - SIGNED)) - 1 + (SIGNED ? 0 : ((unsigned long)1 << (sizeof(T) * 8 - 1))));\
        x = (T)(i < 2 ? i : i < 4 ? hi - (i - 2) : i < 6 ? (T)D + (i - 5) : i < 8 ? (T)(hi / D * D) - (i - 6) : i < 10 ? (T)(hi - D + (i - 8)) : (T)(2 * D - (i - 10)));\
        q = x / D; r = x % D;\
        if (q != x / dv || r != x % dv) ++bad;\
        sum += (unsigned long)q * 31UL + (unsigned long)r;\
    }\
    return bad;\
}

CASE(short, c_short_3, 3, 1)
CASE(short, c_short_7, 7, 1)
CASE(short, c_short_10, 10, 1)
CASE(short, c_short_100, 100, 1)
CASE(short, c_short_1000, 1000, 1)
CASE(short, c_short_32767, 32767, 1)
CASE(unsigned short, c_ushort_3, 3, 0)
CASE(unsigned short, c_ushort_7, 7, 0)
CASE(unsigned short, c_ushort_10, 10, 0)
CASE(unsigned short, c_ushort_100, 100, 0)
CASE(unsigned short, c_ushort_1000, 1000, 0)
CASE(unsigned short, c_ushort_32767, 32767, 0)

int main(void)
{
    long bad = 0;

    bad += c_short_3();
    bad += c_short_7();
    bad += c_short_10();
    bad += c_short_100();
    bad += c_short_1000();
    bad += c_short_32767();
    bad += c_ushort_3();
    bad += c_ushort_7();
    bad += c_ushort_10();
    bad += c_ushort_100();
    bad += c_ushort_1000();
    bad += c_ushort_32767();
    report(bad);
    report((long)sum);
    return 0;
}
