// flags: -O2
// Loops clang turns into ctpop (clear the lowest set bit), cttz and ctlz (shift until a bit shows): the counts reach the backend through the mid-end, not a builtin.
extern void report(long value);

__attribute__((noinline)) int ones32(unsigned x) { int n = 0; for (; x; x &= x - 1) n++; return n; }
__attribute__((noinline)) int ones64(unsigned long long x) { int n = 0; for (; x; x &= x - 1) n++; return n; }
__attribute__((noinline)) int low_zero32(unsigned x) { int n = 0; if (!x) return 32; while (!(x & 1)) { x >>= 1; n++; } return n; }
__attribute__((noinline)) int log2_32(unsigned x) { int n = 0; while (x >>= 1) n++; return n; }
__attribute__((noinline)) int parity16(unsigned short x) { return __builtin_parity(x); }

int main(void)
{
    static const unsigned long long input[] = {1ull, 0x80000000ull, 0x00010000ull, 0xffffffffull, 0x12345678ull, 0x300ull, 0xa5a5a5a5a5a5a5a5ull, 0x100000000ull};
    for (unsigned i = 0; i < sizeof input / sizeof input[0]; i++) {
        volatile unsigned long long x = input[i];
        report(ones32((unsigned)x));
        report(ones64(x));
        report(low_zero32((unsigned)x));
        report(log2_32((unsigned)x));
        report(parity16((unsigned short)x));
    }
    return 0;
}
