// flags: -O2
// bar: none (SDCC error 101: too many parameters)
// clz, ctz and popcount of a 64-bit word, the zero-poison forms and the defined-zero ones (`x ? ... : 32`), each its own function.
extern void report(long value);

__attribute__((noinline)) int lead(unsigned long long x) { return __builtin_clzll(x); }
__attribute__((noinline)) int trail(unsigned long long x) { return __builtin_ctzll(x); }
__attribute__((noinline)) int ones(unsigned long long x) { return __builtin_popcountll(x); }
__attribute__((noinline)) int lead0(unsigned long long x) { return x ? __builtin_clzll(x) : 64; }
__attribute__((noinline)) int trail0(unsigned long long x) { return x ? __builtin_ctzll(x) : 64; }

int main(void)
{
    static const unsigned long long input[] = {1ull, 0x8000000000000000ull, 0x0000100000000000ull, 0xffffffffffffffffull, 0x123456789abcdef0ull, 0x0000000100000000ull, 0x00000000a5a5a5a5ull, 0xa5a5a5a500000000ull};
    for (unsigned i = 0; i < sizeof input / sizeof input[0]; i++) {
        volatile unsigned long long x = input[i];
        report(lead(x));
        report(trail(x));
        report(ones(x));
        report(lead0(x));
        report(trail0(x));
    }
    volatile unsigned long long zero = 0;
    report(ones(zero));
    report(lead0(zero));
    report(trail0(zero));
    return 0;
}
