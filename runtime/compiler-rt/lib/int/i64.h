/* Shared by the 64-bit integer routines, one per file so a link pulls only the routines it calls: 32-bit halves, no 64-bit operation in any body. */
#ifndef SF_I64_H
#define SF_I64_H
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

/* The restoring divide shared by udivdi3/umoddi3/divdi3/moddi3: one copy, not one per file. */
void SFB_NAME(udivmod_w)(uint32_t nlo, uint32_t nhi, uint32_t dlo, uint32_t dhi, uint32_t out[4]);

static inline pair udivmod(pair n, pair d, pair *rem)
{
    uint32_t o[4];
    SFB_NAME(udivmod_w)(n.lo, n.hi, d.lo, d.hi, o);
    pair q = { o[0], o[1] }, r = { o[2], o[3] };
    *rem = r;
    return q;
}

#endif
