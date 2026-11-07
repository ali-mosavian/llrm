#include <stdint.h>

/* i64 multiply by constants (lea/shl chains at m64): each product is reported for several operands. */
extern void report(long value);

__attribute__((noinline)) static long m0(long x){return x*((long)0x0UL);}
__attribute__((noinline)) static long m1(long x){return x*((long)0x1UL);}
__attribute__((noinline)) static long m2(long x){return x*((long)0xffffffffffffffffUL);}
__attribute__((noinline)) static long m3(long x){return x*((long)0x2UL);}
__attribute__((noinline)) static long m4(long x){return x*((long)0x3UL);}
__attribute__((noinline)) static long m5(long x){return x*((long)0x5UL);}
__attribute__((noinline)) static long m6(long x){return x*((long)0x9UL);}
__attribute__((noinline)) static long m7(long x){return x*((long)0xfffffffffffffffdUL);}
__attribute__((noinline)) static long m8(long x){return x*((long)0xfffffffffffffffbUL);}
__attribute__((noinline)) static long m9(long x){return x*((long)0xfffffffffffffff7UL);}
__attribute__((noinline)) static long m10(long x){return x*((long)0x6UL);}
__attribute__((noinline)) static long m11(long x){return x*((long)0xaUL);}
__attribute__((noinline)) static long m12(long x){return x*((long)0x12UL);}
__attribute__((noinline)) static long m13(long x){return x*((long)0xcUL);}
__attribute__((noinline)) static long m14(long x){return x*((long)0x14UL);}
__attribute__((noinline)) static long m15(long x){return x*((long)0x24UL);}
__attribute__((noinline)) static long m16(long x){return x*((long)0x18UL);}
__attribute__((noinline)) static long m17(long x){return x*((long)0x28UL);}
__attribute__((noinline)) static long m18(long x){return x*((long)0x48UL);}
__attribute__((noinline)) static long m19(long x){return x*((long)0xbUL);}
__attribute__((noinline)) static long m20(long x){return x*((long)0x15UL);}
__attribute__((noinline)) static long m21(long x){return x*((long)0x29UL);}
__attribute__((noinline)) static long m22(long x){return x*((long)0x16UL);}
__attribute__((noinline)) static long m23(long x){return x*((long)0x13UL);}
__attribute__((noinline)) static long m24(long x){return x*((long)0x25UL);}
__attribute__((noinline)) static long m25(long x){return x*((long)0x49UL);}
__attribute__((noinline)) static long m26(long x){return x*((long)0xdUL);}
__attribute__((noinline)) static long m27(long x){return x*((long)0x17UL);}
__attribute__((noinline)) static long m28(long x){return x*((long)0x1aUL);}
__attribute__((noinline)) static long m29(long x){return x*((long)0x64UL);}
__attribute__((noinline)) static long m30(long x){return x*((long)0xfUL);}
__attribute__((noinline)) static long m31(long x){return x*((long)0x11UL);}
__attribute__((noinline)) static long m32(long x){return x*((long)0x1fUL);}
__attribute__((noinline)) static long m33(long x){return x*((long)0x21UL);}
__attribute__((noinline)) static long m34(long x){return x*((long)0x3fUL);}
__attribute__((noinline)) static long m35(long x){return x*((long)0x41UL);}
__attribute__((noinline)) static long m36(long x){return x*((long)0x7fUL);}
__attribute__((noinline)) static long m37(long x){return x*((long)0x81UL);}
__attribute__((noinline)) static long m38(long x){return x*((long)0xfffffffffffffff9UL);}
__attribute__((noinline)) static long m39(long x){return x*((long)0xfffffffffffffff1UL);}
__attribute__((noinline)) static long m40(long x){return x*((long)0xffffffffffffffefUL);}
__attribute__((noinline)) static long m41(long x){return x*((long)0xffUL);}
__attribute__((noinline)) static long m42(long x){return x*((long)0x101UL);}
__attribute__((noinline)) static long m43(long x){return x*((long)0xf4243UL);}
__attribute__((noinline)) static long m44(long x){return x*((long)0x7fffffffUL);}
__attribute__((noinline)) static long m45(long x){return x*((long)0x80000000UL);}
__attribute__((noinline)) static long m46(long x){return x*((long)0xffffffffUL);}
__attribute__((noinline)) static long m47(long x){return x*((long)0x100000001UL);}
__attribute__((noinline)) static long m48(long x){return x*((long)0x123456789UL);}
__attribute__((noinline)) static long m49(long x){return x*((long)0xfffffffedcba9877UL);}
__attribute__((noinline)) static long m50(long x){return x*((long)0x7fffffffffffffffUL);}
__attribute__((noinline)) static long m51(long x){return x*((long)0x8000000000000000UL);}
__attribute__((noinline)) static long m52(long x){return x*((long)0x5555555555555555UL);}
__attribute__((noinline)) static long m53(long x){return x*((long)0xaaaaaaaaaaaaaaabUL);}
__attribute__((noinline)) static long m54(long x){return x*((long)0x30000000000UL);}
__attribute__((noinline)) static long m55(long x){return x*((long)0x1200000000UL);}
__attribute__((noinline)) static long m56(long x){return x*((long)0x4000000000000000UL);}

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
// x86-widths: the .out is a 64-bit long (2147483648 and more)
