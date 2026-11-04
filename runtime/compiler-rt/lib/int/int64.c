/* 64-bit multiply and divide on 32-bit halves. */
#include "sf_int.h"
#include "sf_helpers.h"

typedef unsigned long long u64;
typedef long long i64;
typedef struct { uint32_t lo, hi; } pair;

static inline pair split(u64 x)
{
    union { u64 v; pair p; uint32_t w[2]; } u;
    u.v = x;
#if defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
    pair p = { u.w[1], u.w[0] };
#else
    pair p = { u.w[0], u.w[1] };
#endif
    return p;
}

static inline u64 join(pair p)
{
    union { u64 v; uint32_t w[2]; } u;
#if defined(__BYTE_ORDER__) && __BYTE_ORDER__ == __ORDER_BIG_ENDIAN__
    u.w[0] = p.hi; u.w[1] = p.lo;
#else
    u.w[0] = p.lo; u.w[1] = p.hi;
#endif
    return u.v;
}

static inline int ge(pair a, pair b) { return a.hi != b.hi ? a.hi > b.hi : a.lo >= b.lo; }
static inline pair sub(pair a, pair b) { pair r = { a.lo - b.lo, a.hi - b.hi - (a.lo < b.lo) }; return r; }
static inline pair shl(pair a, int n)
{
    if (n >= 32) { pair r = { 0, a.lo << (n - 32) }; return r; }
    if (!n) return a;
    pair r = { a.lo << n, (a.hi << n) | (a.lo >> (32 - n)) };
    return r;
}
static inline pair shr1(pair a) { pair r = { (a.lo >> 1) | (a.hi << 31), a.hi >> 1 }; return r; }
static inline pair neg(pair a) { pair r = { 0u - a.lo, 0u - a.hi - (a.lo != 0) }; return r; }
static inline int clz(pair a) { return a.hi ? sf_clz32(a.hi) : 32 + sf_clz32(a.lo); }

u64 SFB_NAME(muldi3)(u64 x, u64 y)
{
    pair a = split(x), b = split(y), r;
    sf_mul32x32(a.lo, b.lo, &r.hi, &r.lo);
    r.hi += a.lo * b.hi + a.hi * b.lo;
    return join(r);
}

/* Worker takes and returns 32-bit scalars: struct-by-value is not a stable ABI across targets. */
static void udivmod_w(uint32_t nlo, uint32_t nhi, uint32_t dlo, uint32_t dhi, uint32_t out[4])
{
    pair n = { nlo, nhi }, d = { dlo, dhi }, q = { 0, 0 };
    if (!(d.lo | d.hi)) { q.lo = q.hi = ~0u; }
    else if (ge(n, d)) {
        int s = clz(d) - clz(n);
        d = shl(d, s);
        for (int i = 0; i <= s; i++) {
            q = shl(q, 1);
            if (ge(n, d)) { n = sub(n, d); q.lo |= 1; }
            d = shr1(d);
        }
    }
    out[0] = q.lo; out[1] = q.hi; out[2] = n.lo; out[3] = n.hi;
}

static inline __attribute__((always_inline)) pair udivmod(pair n, pair d, pair *rem)
{
    uint32_t o[4];
    udivmod_w(n.lo, n.hi, d.lo, d.hi, o);
    pair q = { o[0], o[1] }, r = { o[2], o[3] };
    *rem = r;
    return q;
}
u64 SFB_NAME(udivdi3)(u64 n, u64 d) { pair r; return join(udivmod(split(n), split(d), &r)); }
u64 SFB_NAME(umoddi3)(u64 n, u64 d) { pair r; udivmod(split(n), split(d), &r); return join(r); }

i64 SFB_NAME(divdi3)(i64 n, i64 d)
{
    pair a = split((u64)n), b = split((u64)d), r;
    int na = (int32_t)a.hi < 0, nb = (int32_t)b.hi < 0;
    pair q = udivmod(na ? neg(a) : a, nb ? neg(b) : b, &r);
    return (i64)join((na != nb) && (b.lo | b.hi) ? neg(q) : q);
}

i64 SFB_NAME(moddi3)(i64 n, i64 d)
{
    pair a = split((u64)n), b = split((u64)d), r;
    int na = (int32_t)a.hi < 0, nb = (int32_t)b.hi < 0;
    udivmod(na ? neg(a) : a, nb ? neg(b) : b, &r);
    return (i64)join(na ? neg(r) : r);
}

/* Shifts: libgcc's (value, int count) signatures. A count of 64 or more gives 0, or the sign for the arithmetic shift. */
u64 SFB_NAME(ashldi3)(u64 x, int n)
{
    pair a = split(x), r = { 0, 0 };
    if ((unsigned)n >= 64) return 0;
    if (n >= 32) r.hi = a.lo << (n - 32);
    else if (n) { r.lo = a.lo << n; r.hi = (a.hi << n) | (a.lo >> (32 - n)); }
    else r = a;
    return join(r);
}

u64 SFB_NAME(lshrdi3)(u64 x, int n)
{
    pair a = split(x), r = { 0, 0 };
    if ((unsigned)n >= 64) return 0;
    if (n >= 32) r.lo = a.hi >> (n - 32);
    else if (n) { r.hi = a.hi >> n; r.lo = (a.lo >> n) | (a.hi << (32 - n)); }
    else r = a;
    return join(r);
}

i64 SFB_NAME(ashrdi3)(i64 x, int n)
{
    pair a = split((u64)x), r;
    uint32_t sign = (uint32_t)((int32_t)a.hi >> 31);
    if ((unsigned)n >= 64) { r.lo = r.hi = sign; }
    else if (n >= 32) { r.lo = (uint32_t)((int32_t)a.hi >> (n - 32)); r.hi = sign; }
    else if (n) { r.hi = (uint32_t)((int32_t)a.hi >> n); r.lo = (a.lo >> n) | (a.hi << (32 - n)); }
    else r = a;
    return (i64)join(r);
}
