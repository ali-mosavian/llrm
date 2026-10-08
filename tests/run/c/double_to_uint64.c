// flags: -O0 -m32 | -O2 -m32
// A float, double or long double converted to an unsigned 64-bit integer, below and from 2^63 (fisttp stores a signed qword and
// answers 0x8000000000000000 for 2^63 and up): "FPToUI to an i64" refused every such function (#829; gcc.c-torture 20040709-1).
extern void report(long value);

typedef unsigned long long u64;

u64 from_double(double d) { return (u64)d; }
u64 from_float(float f) { return (u64)f; }
u64 from_long_double(long double d) { return (u64)d; }

static void show(u64 value)
{
    report((long)(unsigned long)(value >> 32));
    report((long)(unsigned long)value);
}

int main(void)
{
    volatile double d[] = { 0.0, 1.5, 4294967295.0, 4294967296.0, 9223372036854775807.0, 9223372036854775808.0, 9223372036854777856.0, 18446744073709549568.0 };
    volatile float f[] = { 3.9f, 4294967296.0f, 9223372036854775808.0f, 16777216.0f * 1099511627776.0f };
    int i;

    for (i = 0; i < 8; i++)
        show(from_double(d[i]));
    for (i = 0; i < 4; i++)
        show(from_float(f[i]));
    show(from_long_double(18446744073709549568.0L));
    return 0;
}
