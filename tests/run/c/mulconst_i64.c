#include <stdint.h>

/* i64 multiply by constants (lea/shl chains at m64): each product is reported for several operands. */
extern void report(long value);

__attribute__((noinline)) static long m0(long x){return x*(0L);}
__attribute__((noinline)) static long m1(long x){return x*(1L);}
__attribute__((noinline)) static long m2(long x){return x*(-1L);}
__attribute__((noinline)) static long m3(long x){return x*(2L);}
__attribute__((noinline)) static long m4(long x){return x*(3L);}
__attribute__((noinline)) static long m5(long x){return x*(5L);}
__attribute__((noinline)) static long m6(long x){return x*(9L);}
__attribute__((noinline)) static long m7(long x){return x*(-3L);}
__attribute__((noinline)) static long m8(long x){return x*(-5L);}
__attribute__((noinline)) static long m9(long x){return x*(-9L);}
__attribute__((noinline)) static long m10(long x){return x*(6L);}
__attribute__((noinline)) static long m11(long x){return x*(10L);}
__attribute__((noinline)) static long m12(long x){return x*(18L);}
__attribute__((noinline)) static long m13(long x){return x*(12L);}
__attribute__((noinline)) static long m14(long x){return x*(20L);}
__attribute__((noinline)) static long m15(long x){return x*(36L);}
__attribute__((noinline)) static long m16(long x){return x*(24L);}
__attribute__((noinline)) static long m17(long x){return x*(40L);}
__attribute__((noinline)) static long m18(long x){return x*(72L);}
__attribute__((noinline)) static long m19(long x){return x*(11L);}
__attribute__((noinline)) static long m20(long x){return x*(21L);}
__attribute__((noinline)) static long m21(long x){return x*(41L);}
__attribute__((noinline)) static long m22(long x){return x*(22L);}
__attribute__((noinline)) static long m23(long x){return x*(19L);}
__attribute__((noinline)) static long m24(long x){return x*(37L);}
__attribute__((noinline)) static long m25(long x){return x*(73L);}
__attribute__((noinline)) static long m26(long x){return x*(13L);}
__attribute__((noinline)) static long m27(long x){return x*(23L);}
__attribute__((noinline)) static long m28(long x){return x*(26L);}
__attribute__((noinline)) static long m29(long x){return x*(100L);}
__attribute__((noinline)) static long m30(long x){return x*(15L);}
__attribute__((noinline)) static long m31(long x){return x*(17L);}
__attribute__((noinline)) static long m32(long x){return x*(31L);}
__attribute__((noinline)) static long m33(long x){return x*(33L);}
__attribute__((noinline)) static long m34(long x){return x*(63L);}
__attribute__((noinline)) static long m35(long x){return x*(65L);}
__attribute__((noinline)) static long m36(long x){return x*(127L);}
__attribute__((noinline)) static long m37(long x){return x*(129L);}
__attribute__((noinline)) static long m38(long x){return x*(-7L);}
__attribute__((noinline)) static long m39(long x){return x*(-15L);}
__attribute__((noinline)) static long m40(long x){return x*(-17L);}
__attribute__((noinline)) static long m41(long x){return x*(255L);}
__attribute__((noinline)) static long m42(long x){return x*(257L);}
__attribute__((noinline)) static long m43(long x){return x*(1000003L);}
__attribute__((noinline)) static long m44(long x){return x*(2147483647L);}
__attribute__((noinline)) static long m45(long x){return x*(2147483648L);}
__attribute__((noinline)) static long m46(long x){return x*(4294967295L);}
__attribute__((noinline)) static long m47(long x){return x*(4294967297L);}
__attribute__((noinline)) static long m48(long x){return x*(4886718345L);}
__attribute__((noinline)) static long m49(long x){return x*(-4886718345L);}
__attribute__((noinline)) static long m50(long x){return x*(9223372036854775807L);}
__attribute__((noinline)) static long m51(long x){return x*(-9223372036854775807L-1);}
__attribute__((noinline)) static long m52(long x){return x*(6148914691236517205L);}
__attribute__((noinline)) static long m53(long x){return x*(-6148914691236517205L);}
__attribute__((noinline)) static long m54(long x){return x*(3298534883328L);}
__attribute__((noinline)) static long m55(long x){return x*(77309411328L);}
__attribute__((noinline)) static long m56(long x){return x*(23058430092136939520L);}

int main(void) {
    long v[] = {0, 1, -1, 2, 7, -7, 123456789, -987654321, 0x7fffffffffffffffL, -0x7fffffffffffffffL - 1, 0x123456789abcdefL};
    for (unsigned i = 0; i < sizeof v / sizeof *v; i++) {
        volatile long x = v[i];
        report(m0(x));
        report(m1(x));
        report(m2(x));
        report(m3(x));
        report(m4(x));
        report(m5(x));
        report(m6(x));
        report(m7(x));
        report(m8(x));
        report(m9(x));
        report(m10(x));
        report(m11(x));
        report(m12(x));
        report(m13(x));
        report(m14(x));
        report(m15(x));
        report(m16(x));
        report(m17(x));
        report(m18(x));
        report(m19(x));
        report(m20(x));
        report(m21(x));
        report(m22(x));
        report(m23(x));
        report(m24(x));
        report(m25(x));
        report(m26(x));
        report(m27(x));
        report(m28(x));
        report(m29(x));
        report(m30(x));
        report(m31(x));
        report(m32(x));
        report(m33(x));
        report(m34(x));
        report(m35(x));
        report(m36(x));
        report(m37(x));
        report(m38(x));
        report(m39(x));
        report(m40(x));
        report(m41(x));
        report(m42(x));
        report(m43(x));
        report(m44(x));
        report(m45(x));
        report(m46(x));
        report(m47(x));
        report(m48(x));
        report(m49(x));
        report(m50(x));
        report(m51(x));
        report(m52(x));
        report(m53(x));
        report(m54(x));
        report(m55(x));
        report(m56(x));
    }
    return 0;
}
