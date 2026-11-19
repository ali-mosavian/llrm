// flags: -O2
// targets: !int16
// clz, ctz and popcount of a 32-bit word, the zero-poison forms and the defined-zero ones (`x ? ... : 32`), each its own function.
extern void report(long value);

__attribute__((noinline)) int lead(unsigned x) { return __builtin_clz(x); }
__attribute__((noinline)) int trail(unsigned x) { return __builtin_ctz(x); }
__attribute__((noinline)) int ones(unsigned x) { return __builtin_popcount(x); }
__attribute__((noinline)) int lead0(unsigned x) { return x ? __builtin_clz(x) : 32; }
__attribute__((noinline)) int trail0(unsigned x) { return x ? __builtin_ctz(x) : 32; }

int main(void)
{
    static const unsigned input[] = {1u, 0x80000000u, 0x00010000u, 0xffffffffu, 0x12345678u, 0x00000300u, 0xa5a5a5a5u};
    for (unsigned i = 0; i < sizeof input / sizeof input[0]; i++) {
        volatile unsigned x = input[i];
        report(lead(x));
        report(trail(x));
        report(ones(x));
        report(lead0(x));
        report(trail0(x));
    }
    volatile unsigned zero = 0;
    report(ones(zero));
    report(lead0(zero));
    report(trail0(zero));
    return 0;
}
