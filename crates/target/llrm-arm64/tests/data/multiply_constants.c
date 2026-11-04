#include <stdint.h>
#include <stdio.h>
int64_t xm2(int64_t);
int32_t wm2(int32_t);
int64_t xm3(int64_t);
int32_t wm3(int32_t);
int64_t xm5(int64_t);
int32_t wm5(int32_t);
int64_t xm6(int64_t);
int32_t wm6(int32_t);
int64_t xm7(int64_t);
int32_t wm7(int32_t);
int64_t xm9(int64_t);
int32_t wm9(int32_t);
int64_t xm10(int64_t);
int32_t wm10(int32_t);
int64_t xm12(int64_t);
int32_t wm12(int32_t);
int64_t xm15(int64_t);
int32_t wm15(int32_t);
int64_t xm17(int64_t);
int32_t wm17(int32_t);
int64_t xm24(int64_t);
int32_t wm24(int32_t);
int64_t xm28(int64_t);
int32_t wm28(int32_t);
int64_t xm31(int64_t);
int32_t wm31(int32_t);
int64_t xm33(int64_t);
int32_t wm33(int32_t);
int64_t xm40(int64_t);
int32_t wm40(int32_t);
int64_t xm63(int64_t);
int32_t wm63(int32_t);
int64_t xm65(int64_t);
int32_t wm65(int32_t);
int64_t xm100(int64_t);
int32_t wm100(int32_t);
int64_t xm255(int64_t);
int32_t wm255(int32_t);
int64_t xm257(int64_t);
int32_t wm257(int32_t);
int64_t xn1(int64_t);
int32_t wn1(int32_t);
int64_t xn2(int64_t);
int32_t wn2(int32_t);
int64_t xn3(int64_t);
int32_t wn3(int32_t);
int64_t xn5(int64_t);
int32_t wn5(int32_t);
int64_t xn6(int64_t);
int32_t wn6(int32_t);
int64_t xn7(int64_t);
int32_t wn7(int32_t);
int64_t xn9(int64_t);
int32_t wn9(int32_t);
int64_t xn12(int64_t);
int32_t wn12(int32_t);
int64_t xn15(int64_t);
int32_t wn15(int32_t);
int64_t xn28(int64_t);
int32_t wn28(int32_t);
int64_t xn31(int64_t);
int32_t wn31(int32_t);
int64_t xm1000(int64_t);
int32_t wm1000(int32_t);
static const int64_t grid[] = {INT64_MIN, INT64_MIN + 1, -1000000007, -65537, -255, -17, -3, -2, -1, 0, 1, 2, 3, 5, 17, 255, 65537, 1000000007, INT64_MAX - 1, INT64_MAX};
int main(void)
{
    int bad = 0;
    for (unsigned i = 0; i < sizeof grid / sizeof grid[0]; i++) {
        int64_t x = grid[i];
        int32_t y = (int32_t)x;
        if (xm2(x) != (int64_t)((uint64_t)x * (uint64_t)(2LL))) { printf("xm2(%lld)\n", (long long)x); bad = 1; }
        if (wm2(y) != (int32_t)((uint32_t)y * (uint32_t)(2))) { printf("wm2(%d)\n", y); bad = 1; }
        if (xm3(x) != (int64_t)((uint64_t)x * (uint64_t)(3LL))) { printf("xm3(%lld)\n", (long long)x); bad = 1; }
        if (wm3(y) != (int32_t)((uint32_t)y * (uint32_t)(3))) { printf("wm3(%d)\n", y); bad = 1; }
        if (xm5(x) != (int64_t)((uint64_t)x * (uint64_t)(5LL))) { printf("xm5(%lld)\n", (long long)x); bad = 1; }
        if (wm5(y) != (int32_t)((uint32_t)y * (uint32_t)(5))) { printf("wm5(%d)\n", y); bad = 1; }
        if (xm6(x) != (int64_t)((uint64_t)x * (uint64_t)(6LL))) { printf("xm6(%lld)\n", (long long)x); bad = 1; }
        if (wm6(y) != (int32_t)((uint32_t)y * (uint32_t)(6))) { printf("wm6(%d)\n", y); bad = 1; }
        if (xm7(x) != (int64_t)((uint64_t)x * (uint64_t)(7LL))) { printf("xm7(%lld)\n", (long long)x); bad = 1; }
        if (wm7(y) != (int32_t)((uint32_t)y * (uint32_t)(7))) { printf("wm7(%d)\n", y); bad = 1; }
        if (xm9(x) != (int64_t)((uint64_t)x * (uint64_t)(9LL))) { printf("xm9(%lld)\n", (long long)x); bad = 1; }
        if (wm9(y) != (int32_t)((uint32_t)y * (uint32_t)(9))) { printf("wm9(%d)\n", y); bad = 1; }
        if (xm10(x) != (int64_t)((uint64_t)x * (uint64_t)(10LL))) { printf("xm10(%lld)\n", (long long)x); bad = 1; }
        if (wm10(y) != (int32_t)((uint32_t)y * (uint32_t)(10))) { printf("wm10(%d)\n", y); bad = 1; }
        if (xm12(x) != (int64_t)((uint64_t)x * (uint64_t)(12LL))) { printf("xm12(%lld)\n", (long long)x); bad = 1; }
        if (wm12(y) != (int32_t)((uint32_t)y * (uint32_t)(12))) { printf("wm12(%d)\n", y); bad = 1; }
        if (xm15(x) != (int64_t)((uint64_t)x * (uint64_t)(15LL))) { printf("xm15(%lld)\n", (long long)x); bad = 1; }
        if (wm15(y) != (int32_t)((uint32_t)y * (uint32_t)(15))) { printf("wm15(%d)\n", y); bad = 1; }
        if (xm17(x) != (int64_t)((uint64_t)x * (uint64_t)(17LL))) { printf("xm17(%lld)\n", (long long)x); bad = 1; }
        if (wm17(y) != (int32_t)((uint32_t)y * (uint32_t)(17))) { printf("wm17(%d)\n", y); bad = 1; }
        if (xm24(x) != (int64_t)((uint64_t)x * (uint64_t)(24LL))) { printf("xm24(%lld)\n", (long long)x); bad = 1; }
        if (wm24(y) != (int32_t)((uint32_t)y * (uint32_t)(24))) { printf("wm24(%d)\n", y); bad = 1; }
        if (xm28(x) != (int64_t)((uint64_t)x * (uint64_t)(28LL))) { printf("xm28(%lld)\n", (long long)x); bad = 1; }
        if (wm28(y) != (int32_t)((uint32_t)y * (uint32_t)(28))) { printf("wm28(%d)\n", y); bad = 1; }
        if (xm31(x) != (int64_t)((uint64_t)x * (uint64_t)(31LL))) { printf("xm31(%lld)\n", (long long)x); bad = 1; }
        if (wm31(y) != (int32_t)((uint32_t)y * (uint32_t)(31))) { printf("wm31(%d)\n", y); bad = 1; }
        if (xm33(x) != (int64_t)((uint64_t)x * (uint64_t)(33LL))) { printf("xm33(%lld)\n", (long long)x); bad = 1; }
        if (wm33(y) != (int32_t)((uint32_t)y * (uint32_t)(33))) { printf("wm33(%d)\n", y); bad = 1; }
        if (xm40(x) != (int64_t)((uint64_t)x * (uint64_t)(40LL))) { printf("xm40(%lld)\n", (long long)x); bad = 1; }
        if (wm40(y) != (int32_t)((uint32_t)y * (uint32_t)(40))) { printf("wm40(%d)\n", y); bad = 1; }
        if (xm63(x) != (int64_t)((uint64_t)x * (uint64_t)(63LL))) { printf("xm63(%lld)\n", (long long)x); bad = 1; }
        if (wm63(y) != (int32_t)((uint32_t)y * (uint32_t)(63))) { printf("wm63(%d)\n", y); bad = 1; }
        if (xm65(x) != (int64_t)((uint64_t)x * (uint64_t)(65LL))) { printf("xm65(%lld)\n", (long long)x); bad = 1; }
        if (wm65(y) != (int32_t)((uint32_t)y * (uint32_t)(65))) { printf("wm65(%d)\n", y); bad = 1; }
        if (xm100(x) != (int64_t)((uint64_t)x * (uint64_t)(100LL))) { printf("xm100(%lld)\n", (long long)x); bad = 1; }
        if (wm100(y) != (int32_t)((uint32_t)y * (uint32_t)(100))) { printf("wm100(%d)\n", y); bad = 1; }
        if (xm255(x) != (int64_t)((uint64_t)x * (uint64_t)(255LL))) { printf("xm255(%lld)\n", (long long)x); bad = 1; }
        if (wm255(y) != (int32_t)((uint32_t)y * (uint32_t)(255))) { printf("wm255(%d)\n", y); bad = 1; }
        if (xm257(x) != (int64_t)((uint64_t)x * (uint64_t)(257LL))) { printf("xm257(%lld)\n", (long long)x); bad = 1; }
        if (wm257(y) != (int32_t)((uint32_t)y * (uint32_t)(257))) { printf("wm257(%d)\n", y); bad = 1; }
        if (xn1(x) != (int64_t)((uint64_t)x * (uint64_t)(-1LL))) { printf("xn1(%lld)\n", (long long)x); bad = 1; }
        if (wn1(y) != (int32_t)((uint32_t)y * (uint32_t)(-1))) { printf("wn1(%d)\n", y); bad = 1; }
        if (xn2(x) != (int64_t)((uint64_t)x * (uint64_t)(-2LL))) { printf("xn2(%lld)\n", (long long)x); bad = 1; }
        if (wn2(y) != (int32_t)((uint32_t)y * (uint32_t)(-2))) { printf("wn2(%d)\n", y); bad = 1; }
        if (xn3(x) != (int64_t)((uint64_t)x * (uint64_t)(-3LL))) { printf("xn3(%lld)\n", (long long)x); bad = 1; }
        if (wn3(y) != (int32_t)((uint32_t)y * (uint32_t)(-3))) { printf("wn3(%d)\n", y); bad = 1; }
        if (xn5(x) != (int64_t)((uint64_t)x * (uint64_t)(-5LL))) { printf("xn5(%lld)\n", (long long)x); bad = 1; }
        if (wn5(y) != (int32_t)((uint32_t)y * (uint32_t)(-5))) { printf("wn5(%d)\n", y); bad = 1; }
        if (xn6(x) != (int64_t)((uint64_t)x * (uint64_t)(-6LL))) { printf("xn6(%lld)\n", (long long)x); bad = 1; }
        if (wn6(y) != (int32_t)((uint32_t)y * (uint32_t)(-6))) { printf("wn6(%d)\n", y); bad = 1; }
        if (xn7(x) != (int64_t)((uint64_t)x * (uint64_t)(-7LL))) { printf("xn7(%lld)\n", (long long)x); bad = 1; }
        if (wn7(y) != (int32_t)((uint32_t)y * (uint32_t)(-7))) { printf("wn7(%d)\n", y); bad = 1; }
        if (xn9(x) != (int64_t)((uint64_t)x * (uint64_t)(-9LL))) { printf("xn9(%lld)\n", (long long)x); bad = 1; }
        if (wn9(y) != (int32_t)((uint32_t)y * (uint32_t)(-9))) { printf("wn9(%d)\n", y); bad = 1; }
        if (xn12(x) != (int64_t)((uint64_t)x * (uint64_t)(-12LL))) { printf("xn12(%lld)\n", (long long)x); bad = 1; }
        if (wn12(y) != (int32_t)((uint32_t)y * (uint32_t)(-12))) { printf("wn12(%d)\n", y); bad = 1; }
        if (xn15(x) != (int64_t)((uint64_t)x * (uint64_t)(-15LL))) { printf("xn15(%lld)\n", (long long)x); bad = 1; }
        if (wn15(y) != (int32_t)((uint32_t)y * (uint32_t)(-15))) { printf("wn15(%d)\n", y); bad = 1; }
        if (xn28(x) != (int64_t)((uint64_t)x * (uint64_t)(-28LL))) { printf("xn28(%lld)\n", (long long)x); bad = 1; }
        if (wn28(y) != (int32_t)((uint32_t)y * (uint32_t)(-28))) { printf("wn28(%d)\n", y); bad = 1; }
        if (xn31(x) != (int64_t)((uint64_t)x * (uint64_t)(-31LL))) { printf("xn31(%lld)\n", (long long)x); bad = 1; }
        if (wn31(y) != (int32_t)((uint32_t)y * (uint32_t)(-31))) { printf("wn31(%d)\n", y); bad = 1; }
        if (xm1000(x) != (int64_t)((uint64_t)x * (uint64_t)(1000LL))) { printf("xm1000(%lld)\n", (long long)x); bad = 1; }
        if (wm1000(y) != (int32_t)((uint32_t)y * (uint32_t)(1000))) { printf("wm1000(%d)\n", y); bad = 1; }
    }
    return bad;
}
