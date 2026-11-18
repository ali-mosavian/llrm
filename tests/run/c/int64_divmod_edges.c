// flags: -O0 | -O2 | -Os
// 64-bit divide and remainder, signed and unsigned, over the edge operands (zero, one, minus one, the powers of two around 8, 16, 32 and
// 63 bits, the minimum) and 100 pseudo-random pairs of every width, as chained checksums. The hand-written 6502 members and the
// generic C routines must agree on every pair (diff count 0, 2026-11-11). Left out: a zero divisor and INT64_MIN / -1, which C leaves undefined.
extern void report(long value);
typedef __INT32_TYPE__ s32;
typedef __UINT32_TYPE__ u32;
typedef long long s64;
typedef unsigned long long u64;

static const s64 E[] = { 0, 1, -1, 2, -2, 3, 7, 10, 255, 256, 65535, 65536, 0x7FFFFFFFLL, 0x80000000LL, 0xFFFFFFFFLL, 0x100000000LL,
    0x7FFFFFFFFFFFFFFFLL, (s64)0x8000000000000000ULL, (s64)0x8000000000000001ULL, 0x4000000000000000LL, 0x00FF00FF00FF00FFLL,
    -0x100000000LL, 0x123456789ABCDEF0LL, -0x123456789ABCDEFLL };
#define N ((int)(sizeof E / sizeof E[0]))

static u32 mix(u32 h, u64 v) { return h * 31U + (u32)v + (u32)(v >> 32) * 7U; }

static void pair(u32 *h, s64 a, s64 b)
{
    if (b == 0 || (a == (s64)0x8000000000000000ULL && b == -1))
        return;
    h[0] = mix(h[0], (u64)(a / b));
    h[1] = mix(h[1], (u64)(a % b));
    h[2] = mix(h[2], (u64)a / (u64)b);
    h[3] = mix(h[3], (u64)a % (u64)b);
}

int main(void)
{
    volatile s64 va, vb;
    u32 h[4];
    int i, j, k;
    for (i = 0; i < N; ++i) {
        h[0] = h[1] = h[2] = h[3] = 0;
        for (j = 0; j < N; ++j) {
            va = E[i]; vb = E[j];
            pair(h, va, vb);
        }
        for (k = 0; k < 4; ++k) report((s32)h[k]);
    }
    u64 x = 88172645463325252ULL;
    for (i = 0; i < 4; ++i) {
        h[0] = h[1] = h[2] = h[3] = 0;
        for (j = 0; j < 25; ++j) {
            x = x * 6364136223846793005ULL + 1442695040888963407ULL;
            u64 p = x >> ((x >> 58) & 63);
            x = x * 6364136223846793005ULL + 1442695040888963407ULL;
            u64 q = x >> ((x >> 58) & 63);
            va = (s64)p; vb = (s64)q;
            pair(h, va, vb);
        }
        for (k = 0; k < 4; ++k) report((s32)h[k]);
    }
    return 0;
}
