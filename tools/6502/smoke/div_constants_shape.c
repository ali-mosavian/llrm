// flags: -fno-inline-functions
// div_constants_short cut to 100 random numerators a divisor: the quotient and remainder by a constant equal those by a variable divisor
// (a real divide); prints the count of numerators that differ (0) and a checksum of the quotients and remainders.
#include <stdint.h>
extern void report(long value);

static uint32_t state = 2463534242u;
static uint32_t next(void) { state ^= state << 13; state ^= state >> 17; state ^= state << 5; return state; }

static uint32_t sum;

#define CASE(T, NAME, D, SIGNED)\
static long NAME(void)\
{\
    volatile T dv = D;\
    T x, q, r;\
    long bad = 0, i;\
    for (i = 0; i < 100; ++i) {\
        uint32_t w = next(), shape = next() & 7;\
        x = (T)(shape == 0 ? w : shape == 1 ? w >> 16 : shape == 2 ? w >> 24 : shape == 3 ? w >> 29 : w);\
        if (SIGNED && (next() & 1)) x = (T)-x;\
        q = x / D; r = x % D;\
        if (q != x / dv || r != x % dv) ++bad;\
        sum += (uint32_t)q * 31u + (uint32_t)r;\
    }\
    for (i = 0; i < 12; ++i) {\
        T hi = (T)(((uint32_t)1 << (sizeof(T) * 8 - 1 - SIGNED)) - 1 + (SIGNED ? 0 : ((uint32_t)1 << (sizeof(T) * 8 - 1))));\
        x = (T)(i < 2 ? i : i < 4 ? hi - (i - 2) : i < 6 ? (T)D + (i - 5) : i < 8 ? (T)(hi / D * D) - (i - 6) : i < 10 ? (T)(hi - D + (i - 8)) : (T)(2 * D - (i - 10)));\
        q = x / D; r = x % D;\
        if (q != x / dv || r != x % dv) ++bad;\
        sum += (uint32_t)q * 31u + (uint32_t)r;\
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
