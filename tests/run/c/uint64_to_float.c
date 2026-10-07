// flags: -O0 | -O2 | -Os
// An unsigned 64-bit integer made a double or a float (gcc.c-torture 920710-1: "UIToFP from an i64"): at, above and below 2^63, at
// and past 2^53, and 2^64 - 1, which rounds up to 2^64.
extern void report(long value);

typedef unsigned long long u64;

static double to_double(u64 x) { return (double)x; }
static float to_float(u64 x) { return (float)x; }

static long classes;

static void classify(u64 x)
{
    double d = to_double(x);
    float f = to_float(x);
    classes = classes * 3 + (d >= 9223372036854775808.0) + (d >= 18446000000000000000.0) + (f >= 9223372036854775808.0f);
    classes = classes * 3 + (d >= 9007199254740992.0) + (d == 12345.0);
}

int main(void)
{
    classify(0ULL);
    classify(12345ULL);
    classify(9007199254740993ULL);
    classify(9223372036854775808ULL);
    classify(12345678901234567890ULL);
    classify(18446744073709551615ULL);
    report(classes);
    report((long)(to_double(12345678901234567890ULL) / 8589934592.0));
    report((long)(to_double(9223372036854775808ULL) / 8589934592.0));
    report((long)(to_float(9223372036854775808ULL) / 4294967296.0f / 1073741824.0f));
    return 0;
}
