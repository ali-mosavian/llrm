/* The magnitude of a binary32 truncated to 64 bits, shared by fix_di.c and fix_udi.c (one routine each: a program links the one it calls). */
#ifndef SF_F32_FIX_DI_H /* the flatteners paste both files into one unit: one definition */
#define SF_F32_FIX_DI_H
#include "softfloat.h"
#include "sf_helpers.h"

#define SIGN 0x80000000u

/* |a| truncated; *big is set from 2^64 up. */
static inline uint64_t magnitude(sf32 a, int *big)
{
    int e = (int)((a >> 23) & 0xFF) - 127;
    *big = 0;
    if (e < 0) return 0;
    if (e >= 64) { *big = 1; return 0; }
    uint64_t m = (a & 0x7FFFFFu) | 0x800000u;
    return e >= 23 ? m << (e - 23) : m >> (23 - e);
}
#endif
