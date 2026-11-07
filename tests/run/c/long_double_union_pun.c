// flags: -O0 -mlong-double-80 -Wno-incompatible-ms-struct | -O2 -mlong-double-80 -Wno-incompatible-ms-struct | -O0 -m32 -mlong-double-80 -Wno-incompatible-ms-struct | -O2 -m32 -mlong-double-80 -Wno-incompatible-ms-struct
// targets: x86-m16 x86-m32 x86-m64
// The x87 long double read through a union of shorts: its mantissa and its exponent word. The bitcast of the 80-bit
// value to an integer had no promote-result case, and every such pun at -O2 failed to compile on x86_32 and x86_64.
extern void report(long value);

long long make(void)
{
    return 0x0123456789ABCDEFLL;
}

int main(void)
{
    union {
        long double d;
        unsigned short w[5];
    } u;
    u.d = (long double)make();
    for (int i = 0; i < 5; i++)
        report((long)u.w[i]);
    return 0;
}
