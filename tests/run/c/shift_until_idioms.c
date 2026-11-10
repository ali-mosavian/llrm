// flags: -O1 | -O2 | -O3 | -Os
// Loops LoopIdiomRecognize makes countable: shift until a bit is set (variable or constant mask), shift a value by a growing amount until it is zero, shift until below a threshold.
extern void report(long value); /* long is 32 bits on wasm32 and 64 natively: report ints */

__attribute__((noinline)) unsigned until_bit(unsigned x, unsigned pos) { unsigned m = 1u << pos; while (!(x & m)) x <<= 1; return x; }
__attribute__((noinline)) unsigned until_bit8(unsigned x) { while (!(x & 256u)) x <<= 1; return x; }
__attribute__((noinline)) unsigned until_bit8_next(unsigned x) { unsigned y; do { y = x << 1; x = y; } while (!(x & 256u)); return x; }

__attribute__((noinline)) int offset_count(unsigned val, int start, int off) { int iv; for (iv = start; (val >> (iv + off)) != 0; iv++) ; return iv; }
__attribute__((noinline)) int shl_count(unsigned val) { int iv; for (iv = 0; (val << iv) != 0; iv++) ; return iv; }
__attribute__((noinline)) int below_four(unsigned x) { int n = 0; if (x < 2) return 0; do { x >>= 1; n++; } while (x >= 4); return n; }
__attribute__((noinline)) int below_two(unsigned x) { int n = 0; do { n++; x >>= 1; } while (x >= 2); return n; }

int main(void)
{
    static const unsigned xs[] = {1u, 2u, 0x10u, 0x100u, 0x1ffu, 0x12345678u, 0x80000000u, 0xffffffffu, 0x00ff00u, 0x41u};
    static const unsigned positions[] = {0u, 3u, 8u, 15u, 31u};
    for (unsigned i = 0; i < sizeof xs / sizeof xs[0]; i++) {
        volatile unsigned x = xs[i];
        for (unsigned j = 0; j < sizeof positions / sizeof positions[0]; j++) {
            unsigned pos = positions[j];
            unsigned low = pos == 31 ? ~0u : (2u << pos) - 1u;
            if (x & low) report((int)until_bit(x, pos));
        }
        if (x & 0x1ffu) report((int)until_bit8(x));
        if (x & 0xffu) report((int)until_bit8_next(x));
    }
    static const unsigned vals[] = {1u, 0x10u, 0xffu, 0x7fffffffu, 0x1234u, 0u};
    static const int starts[] = {0, 2, 5};
    static const int offs[] = {0, 1, 3};
    for (unsigned i = 0; i < sizeof vals / sizeof vals[0]; i++) {
        volatile unsigned v = vals[i];
        for (unsigned j = 0; j < 3; j++)
            for (unsigned k = 0; k < 3; k++) report(offset_count(v, starts[j], offs[k]));
        report(below_four(v));
        report(below_two(v));
    }
    static const unsigned evens[] = {0u, 2u, 0x10u, 0x80000000u, 0xfffffff0u};
    for (unsigned i = 0; i < sizeof evens / sizeof evens[0]; i++) {
        volatile unsigned v = evens[i];
        report(shl_count(v));
    }
    return 0;
}
