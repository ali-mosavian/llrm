#include <stdint.h>

/* Sign-bit idioms at i8..i64; each line reports the six results packed 2 bits apiece (value + 1). */
extern void report(long value);

#define IDIOMS(T, U, N, SIGN) \
__attribute__((noinline)) static int ge_##N(T x, int y) { return (x >= 0) + y; } \
__attribute__((noinline)) static int lt_##N(T x, int y) { return (x < 0) + y; } \
__attribute__((noinline)) static int bit_##N(T x, int y) { return ((x & SIGN) == 0) + y; } \
__attribute__((noinline)) static int bitne_##N(T x, int y) { return ((x & SIGN) != 0) + y; } \
__attribute__((noinline)) static int ubit_##N(U x, int y) { return ((x & (U)SIGN) == 0) + y; } \
__attribute__((noinline)) static int lowbit_##N(T x, int y) { return ((x & 1) == 0) + y; } \
static void run_##N(void) { \
    T v[] = {0, 1, -1, (T)SIGN, (T)((U)SIGN - 1), (T)((U)SIGN + 1), (T)(~SIGN), 5, -5}; \
    for (unsigned i = 0; i < sizeof v / sizeof *v; i++) { volatile T x = v[i]; \
        for (int y = -1; y <= 1; y++) { volatile int yy = y; \
            report((((((ge_##N(x, yy) + 1L) * 4 + lt_##N(x, yy) + 1) * 4 + bit_##N(x, yy) + 1) * 4 + bitne_##N(x, yy) + 1) * 4 \
                    + ubit_##N((U)x, yy) + 1) * 4 + lowbit_##N(x, yy) + 1); } } }

IDIOMS(int8_t, uint8_t, i8, INT8_MIN)
IDIOMS(int16_t, uint16_t, i16, INT16_MIN)
IDIOMS(int32_t, uint32_t, i32, INT32_MIN)
IDIOMS(int64_t, uint64_t, i64, INT64_MIN)

int main(void) { run_i8(); run_i16(); run_i32(); run_i64(); return 0; }
