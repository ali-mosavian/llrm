// flags: -O0 | -O2 | -Os
// bar: none (SDCC links ___fs2slonglong: no 64-bit float conversion)
// A double or float converted to a signed 64-bit integer (gcc.c-torture 930622-2, pr49218: "FPToSI to an i64"), truncating
// toward zero, over values below, at and above 2^32. The unsigned conversion is #829's other half.
#include <stdint.h>
// The words of a 64-bit value by significance, not by index: the low word is word[0] only on a little-endian target.
#define LOW (__BYTE_ORDER__ == __ORDER_BIG_ENDIAN__)
#define HIGH (!LOW)
extern void report(long value);

typedef long long s64;

union halves { s64 whole; uint32_t word[2]; };

static s64 trunc_double(double d) { return (s64)d; }
static s64 trunc_float(float f) { return (s64)f; }

int main(void)
{
    static const double values[] = { 0.0, 1.9, -1.9, 123456.75, -123456.75, 4294967296.5, -4294967297.5, 1099511627776.0, -5000000000000.0 };
    long low = 0, high = 0;
    int i;
    for (i = 0; i < 9; ++i) {
        union halves t;
        t.whole = trunc_double(values[i]);
        low = low * 31 + (long)t.word[LOW];
        high = high * 31 + (long)t.word[HIGH];
    }
    report(low);
    report(high);
    {
        union halves big, small;
        big.whole = trunc_float(65536.0f * 65536.0f * 8.0f);
        small.whole = trunc_float(-3.75f);
        report((long)(big.word[HIGH] << 2 | big.word[LOW] >> 30));
        report((long)small.word[LOW]);
    }
    return 0;
}
