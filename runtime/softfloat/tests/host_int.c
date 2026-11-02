/* Integer builtins vs C integer semantics. Division by zero follows sf_int.h. */
/* cc -O2 -fno-builtin -Iinclude '-DSFB_NAME(n)=sfb_##n' tests/host_int.c src/int32.c src/int64.c */
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
typedef unsigned long long u64; typedef long long i64;
uint32_t sfb_mulsi3(uint32_t, uint32_t); uint32_t sfb_udivsi3(uint32_t, uint32_t); uint32_t sfb_umodsi3(uint32_t, uint32_t);
int32_t sfb_divsi3(int32_t, int32_t); int32_t sfb_modsi3(int32_t, int32_t);
u64 sfb_muldi3(u64, u64); u64 sfb_udivdi3(u64, u64); u64 sfb_umoddi3(u64, u64);
i64 sfb_divdi3(i64, i64); i64 sfb_moddi3(i64, i64);

static u64 rng = 0x2545F4914F6CDD1Dull;
static u64 rnd(void) { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; return rng; }
static u64 gen(void)
{
    u64 r = rnd();
    static const u64 e[] = {0, 1, 2, 3, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF, 0x100000000ull, 0x7FFFFFFFFFFFFFFFull,
                            0x8000000000000000ull, 0xFFFFFFFFFFFFFFFFull, 0xFFFF, 0x10000, 0xFFFF0000};
    switch (r & 3) {
    case 0: return e[(r >> 8) % (sizeof e / sizeof *e)];
    case 1: return rnd() >> (rnd() & 63);
    case 2: return -(rnd() >> (rnd() & 63));
    default: return rnd();
    }
}
static long fails, n;
#define CK(name, got, want, a, b) do { n++; if ((got) != (want) && fails++ < 30) \
    printf("FAIL %s a=%llx b=%llx got=%llx want=%llx\n", name, (u64)(a), (u64)(b), (u64)(got), (u64)(want)); } while (0)

int main(int argc, char **argv)
{
    long cnt = argc > 1 ? atol(argv[1]) : 5000000;
    for (long i = 0; i < cnt; i++) {
        u64 a = gen(), b = gen();
        uint32_t x = (uint32_t)a, y = (uint32_t)b;
        CK("mulsi3", sfb_mulsi3(x, y), x * y, x, y);
        CK("muldi3", sfb_muldi3(a, b), a * b, a, b);
        u64 q = ~0ull;
        CK("udivsi3", sfb_udivsi3(x, y), y ? x / y : ~0u, x, y);
        CK("umodsi3", sfb_umodsi3(x, y), y ? x % y : x, x, y);
        CK("udivdi3", sfb_udivdi3(a, b), b ? a / b : q, a, b);
        CK("umoddi3", sfb_umoddi3(a, b), b ? a % b : a, a, b);
        int32_t sx = (int32_t)x, sy = (int32_t)y;
        int32_t dq = !sy ? -1 : (sx == INT32_MIN && sy == -1) ? INT32_MIN : sx / sy;
        int32_t dr = !sy ? sx : (sx == INT32_MIN && sy == -1) ? 0 : sx % sy;
        CK("divsi3", sfb_divsi3(sx, sy), dq, x, y);
        CK("modsi3", sfb_modsi3(sx, sy), dr, x, y);
        i64 la = (i64)a, lb = (i64)b;
        i64 lq = !lb ? -1 : (la == INT64_MIN && lb == -1) ? INT64_MIN : la / lb;
        i64 lr = !lb ? la : (la == INT64_MIN && lb == -1) ? 0 : la % lb;
        CK("divdi3", sfb_divdi3(la, lb), lq, a, b);
        CK("moddi3", sfb_moddi3(la, lb), lr, a, b);
    }
    printf("vectors=%ld per op=%ld fails=%ld\n", n, cnt, fails);
    return fails != 0;
}
