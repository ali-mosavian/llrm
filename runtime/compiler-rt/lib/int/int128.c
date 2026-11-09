/* 128-bit shifts and the product: a target whose backend lowers a variable i128 shift to a libcall (wasm32) needs these under the compiler-rt names; the soft-float binary128
 * code shifts variably. The 128-bit value is split into 64-bit halves so no body shifts a 128-bit value by a variable (it would call itself). */
#include "sf_int.h"

#ifdef __SIZEOF_INT128__
typedef unsigned __int128 sf_tu;
typedef __int128 sf_ti;
typedef union { sf_tu all; struct { uint64_t lo, hi; } s; } sf_t128; /* little-endian halves, as the targets that use this */

sf_tu SFB_NAME(ashlti3)(sf_tu a, int b)
{
    sf_t128 x; x.all = a;
    if (b & 64) { x.s.hi = x.s.lo << (b & 63); x.s.lo = 0; }
    else if (b) { x.s.hi = (x.s.hi << b) | (x.s.lo >> (64 - b)); x.s.lo <<= b; }
    return x.all;
}

sf_tu SFB_NAME(lshrti3)(sf_tu a, int b)
{
    sf_t128 x; x.all = a;
    if (b & 64) { x.s.lo = x.s.hi >> (b & 63); x.s.hi = 0; }
    else if (b) { x.s.lo = (x.s.lo >> b) | (x.s.hi << (64 - b)); x.s.hi >>= b; }
    return x.all;
}

sf_ti SFB_NAME(ashrti3)(sf_ti a, int b)
{
    sf_t128 x; x.all = (sf_tu)a;
    int64_t hi = (int64_t)x.s.hi;
    if (b & 64) { x.s.lo = (uint64_t)(hi >> (b & 63)); x.s.hi = (uint64_t)(hi >> 63); }
    else if (b) { x.s.lo = (x.s.lo >> b) | ((uint64_t)hi << (64 - b)); x.s.hi = (uint64_t)(hi >> b); }
    return (sf_ti)x.all;
}
/* The 128-bit product of two 64-bit values, from 32-bit partials: no operation here widens, so none calls __multi3. */
static sf_tu mul64x64(uint64_t a, uint64_t b)
{
    uint64_t al = (uint32_t)a, ah = a >> 32, bl = (uint32_t)b, bh = b >> 32;
    uint64_t ll = al * bl, lh = al * bh, hl = ah * bl, hh = ah * bh;
    uint64_t mid = (ll >> 32) + (uint32_t)lh + (uint32_t)hl;
    sf_t128 r;
    r.s.lo = (mid << 32) | (uint32_t)ll;
    r.s.hi = hh + (lh >> 32) + (hl >> 32) + (mid >> 32);
    return r.all;
}

/* The low 128 bits of the product (signed and unsigned agree). wasm32 lowers an i128 multiply to this call. */
sf_ti SFB_NAME(multi3)(sf_ti a, sf_ti b)
{
    sf_t128 x, y, r;
    x.all = (sf_tu)a; y.all = (sf_tu)b;
    r.all = mul64x64(x.s.lo, y.s.lo);
    r.s.hi += x.s.hi * y.s.lo + x.s.lo * y.s.hi;
    return (sf_ti)r.all;
}
#endif
