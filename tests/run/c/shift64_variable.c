// flags: -O1 | -O2 | -Os
// A 64-bit value shifted by a count the compiler cannot see: left, logical right and arithmetic right, by 0, 1, 8, 31, 32, 33 and 63.
#include <stdint.h>
extern void report(long value);

__attribute__((noinline)) uint64_t shl(uint64_t x, int n) { return x << n; }
__attribute__((noinline)) uint64_t shr(uint64_t x, int n) { return x >> n; }
__attribute__((noinline)) int64_t sar(int64_t x, int n) { return x >> n; }

static void put(uint64_t v)
{
    report((long)(int32_t)(uint32_t)(v >> 32));
    report((long)(int32_t)(uint32_t)v);
}

int main(void)
{
    static const uint64_t v[] = {0x123456789ABCDEF0ull, 0x8000000000000001ull, 0xFFFFFFFFFFFFFFFFull, 0x00FF00FF0F0F0F0Full};
    static const int counts[] = {0, 1, 2, 7, 8, 9, 15, 16, 31, 32, 33, 47, 48, 63};
    volatile int n;
    for (unsigned i = 0; i < sizeof v / sizeof v[0]; i++)
        for (unsigned j = 0; j < sizeof counts / sizeof counts[0]; j++) {
            n = counts[j];
            put(shl(v[i], n));
            put(shr(v[i], n));
            put((uint64_t)sar((int64_t)v[i], n));
        }
    return 0;
}
