// flags: -O1 | -O2 | -O3 | -Os
// Loops LoopIdiomRecognize makes countable: shift until a bit is set (variable or constant mask), shift a value by a growing amount until it is zero, shift until below a threshold.
#include <stdint.h>
extern void report(long value); /* long is 32 bits on wasm32 and 64 natively; int is 16 bits on z80/m16: values are uint32_t/int32_t */

__attribute__((noinline)) uint32_t until_bit(uint32_t x, uint32_t pos) { uint32_t m = (uint32_t)1 << pos; while (!(x & m)) x <<= 1; return x; }
__attribute__((noinline)) uint32_t until_bit8(uint32_t x) { while (!(x & (uint32_t)256)) x <<= 1; return x; }
__attribute__((noinline)) uint32_t until_bit8_next(uint32_t x) { uint32_t y; do { y = x << 1; x = y; } while (!(x & (uint32_t)256)); return x; }

__attribute__((noinline)) int32_t offset_count(uint32_t val, int start, int off) { int32_t iv; for (iv = start; (val >> (iv + off)) != 0; iv++) ; return iv; }
__attribute__((noinline)) int32_t shl_count(uint32_t val) { int32_t iv; for (iv = 0; (val << iv) != 0; iv++) ; return iv; }
__attribute__((noinline)) int32_t below_four(uint32_t x) { int32_t n = 0; if (x < 2) return 0; do { x >>= 1; n++; } while (x >= 4); return n; }
__attribute__((noinline)) int32_t below_two(uint32_t x) { int32_t n = 0; do { n++; x >>= 1; } while (x >= 2); return n; }

int main(void)
{
    static const uint32_t xs[] = {1u, 2u, 0x10u, 0x100u, 0x1ffu, 0x12345678u, 0x80000000u, 0xffffffffu, 0x00ff00u, 0x41u};
    static const uint32_t positions[] = {0u, 3u, 8u, 15u, 31u};
    for (unsigned i = 0; i < sizeof xs / sizeof xs[0]; i++) {
        volatile uint32_t x = xs[i];
        for (unsigned j = 0; j < sizeof positions / sizeof positions[0]; j++) {
            uint32_t pos = positions[j];
            uint32_t low = pos == 31 ? ~(uint32_t)0 : ((uint32_t)2 << pos) - 1u;
            if (x & low) report((int32_t)until_bit(x, pos));
        }
        if (x & 0x1ffu) report((int32_t)until_bit8(x));
        if (x & 0xffu) report((int32_t)until_bit8_next(x));
    }
    static const uint32_t vals[] = {1u, 0x10u, 0xffu, 0x7fffffffu, 0x1234u, 0u};
    static const int32_t starts[] = {0, 2, 5};
    static const int32_t offs[] = {0, 1, 3};
    for (unsigned i = 0; i < sizeof vals / sizeof vals[0]; i++) {
        volatile uint32_t v = vals[i];
        for (unsigned j = 0; j < 3; j++)
            for (unsigned k = 0; k < 3; k++) report(offset_count(v, starts[j], offs[k]));
        report(below_four(v));
        report(below_two(v));
    }
    static const uint32_t evens[] = {0u, 2u, 0x10u, 0x80000000u, 0xfffffff0u};
    for (unsigned i = 0; i < sizeof evens / sizeof evens[0]; i++) {
        volatile uint32_t v = evens[i];
        report(shl_count(v));
    }
    return 0;
}
