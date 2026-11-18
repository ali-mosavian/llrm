// flags: -O1 | -O2 | -Os
// A float and a double converted to u64, at every exponent that shifts the mantissa right (1..23, 1..52 places), left (to 2^63 and beyond)
// and the ones that do not shift it, plus the values below 1 and the largest below 2^64: the 6502's hand-written members against the C result.
#include <stdint.h>
extern void report(long value);

static void put(uint64_t v)
{
    report((long)(int32_t)(uint32_t)(v >> 32));
    report((long)(int32_t)(uint32_t)v);
}

__attribute__((noinline)) uint64_t from_float(float f) { return (uint64_t)f; }
__attribute__((noinline)) uint64_t from_double(double d) { return (uint64_t)d; }

int main(void)
{
    static const float f[] = {
        0.0f, 0.5f, 0.99999994f, 1.0f, 1.5f, 3.75f, 255.5f, 12345.678f, 65535.0f, 8388607.5f, 8388608.0f, 16777215.0f, 16777216.0f,
        33554432.0f, 1.0e10f, 4294967296.0f, 1099511627776.0f, 9.0e15f, 9223372036854775808.0f, 1.2e19f, 18446742974197923840.0f };
    static const double d[] = {
        0.0, 0.5, 0.9999999999999999, 1.0, 1.5, 3.75, 12345.678, 4503599627370495.5, 4503599627370496.0, 4503599627370497.0,
        9007199254740991.0, 9007199254740992.0, 9007199254740994.0, 1.0e16, 1.5e17, 4611686018427387904.0, 9223372036854774784.0,
        9223372036854775808.0, 12345678901234567890.0, 18446744073709549568.0 };
    int i;

    for (i = 0; i < (int)(sizeof f / sizeof f[0]); i++)
        put(from_float(((volatile const float *)f)[i]));
    for (i = 0; i < (int)(sizeof d / sizeof d[0]); i++)
        put(from_double(((volatile const double *)d)[i]));
    return 0;
}
