/* 64x64 -> 128 for the multiplication and the Newton division. */
#ifndef SF_MUL64_H
#define SF_MUL64_H

#define LOW32 0xFFFFFFFFull

/* 64x64 -> 128 as hi:lo: SF_MUL64X64 when the target has it, else 32x32 partial products. */
#ifdef SF_MUL64X64
#define mul64(a, b, hi, lo) SF_MUL64X64(a, b, hi, lo)
#else
static inline void mul64(uint64_t a, uint64_t b, uint64_t *hi, uint64_t *lo)
{
    uint32_t h, l;
    uint64_t ll, lh, hl, hh, mid;
    sf_mul32x32((uint32_t)a, (uint32_t)b, &h, &l); ll = (uint64_t)h << 32 | l;
    sf_mul32x32((uint32_t)a, (uint32_t)(b >> 32), &h, &l); lh = (uint64_t)h << 32 | l;
    sf_mul32x32((uint32_t)(a >> 32), (uint32_t)b, &h, &l); hl = (uint64_t)h << 32 | l;
    sf_mul32x32((uint32_t)(a >> 32), (uint32_t)(b >> 32), &h, &l); hh = (uint64_t)h << 32 | l;
    mid = (ll >> 32) + (lh & LOW32) + (hl & LOW32);
    *lo = (ll & LOW32) | (mid << 32);
    *hi = hh + (lh >> 32) + (hl >> 32) + (mid >> 32);
}
#endif

#endif
