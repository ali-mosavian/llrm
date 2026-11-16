// flags: -O1 | -O2 | -O3 | -Os
// A 32- and 64-bit value shifted by every constant: each result is reported, so a shift that drops or repeats a bit shows.
#include <stdint.h>
extern void report(long value);

__attribute__((noinline)) uint32_t l1(uint32_t x) { return x << 1; }
__attribute__((noinline)) uint32_t r1(uint32_t x) { return x >> 1; }
__attribute__((noinline)) uint32_t l2(uint32_t x) { return x << 2; }
__attribute__((noinline)) uint32_t r2(uint32_t x) { return x >> 2; }
__attribute__((noinline)) uint32_t l3(uint32_t x) { return x << 3; }
__attribute__((noinline)) uint32_t r3(uint32_t x) { return x >> 3; }
__attribute__((noinline)) uint32_t l4(uint32_t x) { return x << 4; }
__attribute__((noinline)) uint32_t r4(uint32_t x) { return x >> 4; }
__attribute__((noinline)) uint32_t l5(uint32_t x) { return x << 5; }
__attribute__((noinline)) uint32_t r5(uint32_t x) { return x >> 5; }
__attribute__((noinline)) uint32_t l6(uint32_t x) { return x << 6; }
__attribute__((noinline)) uint32_t r6(uint32_t x) { return x >> 6; }
__attribute__((noinline)) uint32_t l7(uint32_t x) { return x << 7; }
__attribute__((noinline)) uint32_t r7(uint32_t x) { return x >> 7; }
__attribute__((noinline)) uint32_t l8(uint32_t x) { return x << 8; }
__attribute__((noinline)) uint32_t r8(uint32_t x) { return x >> 8; }
__attribute__((noinline)) uint32_t l9(uint32_t x) { return x << 9; }
__attribute__((noinline)) uint32_t r9(uint32_t x) { return x >> 9; }
__attribute__((noinline)) uint32_t l10(uint32_t x) { return x << 10; }
__attribute__((noinline)) uint32_t r10(uint32_t x) { return x >> 10; }
__attribute__((noinline)) uint32_t l11(uint32_t x) { return x << 11; }
__attribute__((noinline)) uint32_t r11(uint32_t x) { return x >> 11; }
__attribute__((noinline)) uint32_t l12(uint32_t x) { return x << 12; }
__attribute__((noinline)) uint32_t r12(uint32_t x) { return x >> 12; }
__attribute__((noinline)) uint32_t l13(uint32_t x) { return x << 13; }
__attribute__((noinline)) uint32_t r13(uint32_t x) { return x >> 13; }
__attribute__((noinline)) uint32_t l14(uint32_t x) { return x << 14; }
__attribute__((noinline)) uint32_t r14(uint32_t x) { return x >> 14; }
__attribute__((noinline)) uint32_t l15(uint32_t x) { return x << 15; }
__attribute__((noinline)) uint32_t r15(uint32_t x) { return x >> 15; }
__attribute__((noinline)) uint32_t l16(uint32_t x) { return x << 16; }
__attribute__((noinline)) uint32_t r16(uint32_t x) { return x >> 16; }
__attribute__((noinline)) uint32_t l17(uint32_t x) { return x << 17; }
__attribute__((noinline)) uint32_t r17(uint32_t x) { return x >> 17; }
__attribute__((noinline)) uint32_t l18(uint32_t x) { return x << 18; }
__attribute__((noinline)) uint32_t r18(uint32_t x) { return x >> 18; }
__attribute__((noinline)) uint32_t l19(uint32_t x) { return x << 19; }
__attribute__((noinline)) uint32_t r19(uint32_t x) { return x >> 19; }
__attribute__((noinline)) uint32_t l20(uint32_t x) { return x << 20; }
__attribute__((noinline)) uint32_t r20(uint32_t x) { return x >> 20; }
__attribute__((noinline)) uint32_t l21(uint32_t x) { return x << 21; }
__attribute__((noinline)) uint32_t r21(uint32_t x) { return x >> 21; }
__attribute__((noinline)) uint32_t l22(uint32_t x) { return x << 22; }
__attribute__((noinline)) uint32_t r22(uint32_t x) { return x >> 22; }
__attribute__((noinline)) uint32_t l23(uint32_t x) { return x << 23; }
__attribute__((noinline)) uint32_t r23(uint32_t x) { return x >> 23; }
__attribute__((noinline)) uint32_t l24(uint32_t x) { return x << 24; }
__attribute__((noinline)) uint32_t r24(uint32_t x) { return x >> 24; }
__attribute__((noinline)) uint32_t l25(uint32_t x) { return x << 25; }
__attribute__((noinline)) uint32_t r25(uint32_t x) { return x >> 25; }
__attribute__((noinline)) uint32_t l26(uint32_t x) { return x << 26; }
__attribute__((noinline)) uint32_t r26(uint32_t x) { return x >> 26; }
__attribute__((noinline)) uint32_t l27(uint32_t x) { return x << 27; }
__attribute__((noinline)) uint32_t r27(uint32_t x) { return x >> 27; }
__attribute__((noinline)) uint32_t l28(uint32_t x) { return x << 28; }
__attribute__((noinline)) uint32_t r28(uint32_t x) { return x >> 28; }
__attribute__((noinline)) uint32_t l29(uint32_t x) { return x << 29; }
__attribute__((noinline)) uint32_t r29(uint32_t x) { return x >> 29; }
__attribute__((noinline)) uint32_t l30(uint32_t x) { return x << 30; }
__attribute__((noinline)) uint32_t r30(uint32_t x) { return x >> 30; }
__attribute__((noinline)) uint32_t l31(uint32_t x) { return x << 31; }
__attribute__((noinline)) uint32_t r31(uint32_t x) { return x >> 31; }
__attribute__((noinline)) uint64_t L1(uint64_t x) { return x << 1; }
__attribute__((noinline)) uint64_t R1(uint64_t x) { return x >> 1; }
__attribute__((noinline)) uint64_t L2(uint64_t x) { return x << 2; }
__attribute__((noinline)) uint64_t R2(uint64_t x) { return x >> 2; }
__attribute__((noinline)) uint64_t L3(uint64_t x) { return x << 3; }
__attribute__((noinline)) uint64_t R3(uint64_t x) { return x >> 3; }
__attribute__((noinline)) uint64_t L4(uint64_t x) { return x << 4; }
__attribute__((noinline)) uint64_t R4(uint64_t x) { return x >> 4; }
__attribute__((noinline)) uint64_t L5(uint64_t x) { return x << 5; }
__attribute__((noinline)) uint64_t R5(uint64_t x) { return x >> 5; }
__attribute__((noinline)) uint64_t L6(uint64_t x) { return x << 6; }
__attribute__((noinline)) uint64_t R6(uint64_t x) { return x >> 6; }
__attribute__((noinline)) uint64_t L7(uint64_t x) { return x << 7; }
__attribute__((noinline)) uint64_t R7(uint64_t x) { return x >> 7; }

static void put64(uint64_t v) { report((long)(int32_t)(uint32_t)v); report((long)(int32_t)(uint32_t)(v >> 32)); }

int main(void)
{
    static const uint32_t v[] = {0x12345678u, 0x9ABCDEF0u, 0x80000001u, 0xFFFFFFFFu, 1u, 0x0F0F00FFu};
    for (unsigned i = 0; i < sizeof v / sizeof v[0]; i++) {
        uint32_t x = v[i];
        report((long)(int32_t)l1(x));
        report((long)(int32_t)r1(x));
        report((long)(int32_t)l2(x));
        report((long)(int32_t)r2(x));
        report((long)(int32_t)l3(x));
        report((long)(int32_t)r3(x));
        report((long)(int32_t)l4(x));
        report((long)(int32_t)r4(x));
        report((long)(int32_t)l5(x));
        report((long)(int32_t)r5(x));
        report((long)(int32_t)l6(x));
        report((long)(int32_t)r6(x));
        report((long)(int32_t)l7(x));
        report((long)(int32_t)r7(x));
        report((long)(int32_t)l8(x));
        report((long)(int32_t)r8(x));
        report((long)(int32_t)l9(x));
        report((long)(int32_t)r9(x));
        report((long)(int32_t)l10(x));
        report((long)(int32_t)r10(x));
        report((long)(int32_t)l11(x));
        report((long)(int32_t)r11(x));
        report((long)(int32_t)l12(x));
        report((long)(int32_t)r12(x));
        report((long)(int32_t)l13(x));
        report((long)(int32_t)r13(x));
        report((long)(int32_t)l14(x));
        report((long)(int32_t)r14(x));
        report((long)(int32_t)l15(x));
        report((long)(int32_t)r15(x));
        report((long)(int32_t)l16(x));
        report((long)(int32_t)r16(x));
        report((long)(int32_t)l17(x));
        report((long)(int32_t)r17(x));
        report((long)(int32_t)l18(x));
        report((long)(int32_t)r18(x));
        report((long)(int32_t)l19(x));
        report((long)(int32_t)r19(x));
        report((long)(int32_t)l20(x));
        report((long)(int32_t)r20(x));
        report((long)(int32_t)l21(x));
        report((long)(int32_t)r21(x));
        report((long)(int32_t)l22(x));
        report((long)(int32_t)r22(x));
        report((long)(int32_t)l23(x));
        report((long)(int32_t)r23(x));
        report((long)(int32_t)l24(x));
        report((long)(int32_t)r24(x));
        report((long)(int32_t)l25(x));
        report((long)(int32_t)r25(x));
        report((long)(int32_t)l26(x));
        report((long)(int32_t)r26(x));
        report((long)(int32_t)l27(x));
        report((long)(int32_t)r27(x));
        report((long)(int32_t)l28(x));
        report((long)(int32_t)r28(x));
        report((long)(int32_t)l29(x));
        report((long)(int32_t)r29(x));
        report((long)(int32_t)l30(x));
        report((long)(int32_t)r30(x));
        report((long)(int32_t)l31(x));
        report((long)(int32_t)r31(x));
    }
    static const uint64_t w[] = {0x123456789ABCDEF0ull, 0x8000000000000001ull, 0xFFFFFFFFFFFFFFFFull, 0x00FF00FF0F0F0F0Full};
    for (unsigned i = 0; i < sizeof w / sizeof w[0]; i++) {
        uint64_t x = w[i];
        put64(L1(x));
        put64(R1(x));
        put64(L2(x));
        put64(R2(x));
        put64(L3(x));
        put64(R3(x));
        put64(L4(x));
        put64(R4(x));
        put64(L5(x));
        put64(R5(x));
        put64(L6(x));
        put64(R6(x));
        put64(L7(x));
        put64(R7(x));
    }
    return 0;
}
