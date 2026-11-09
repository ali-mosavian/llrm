/* __clzdi2: leading zeros of a non-zero 64-bit value, from the 32-bit halves. */
#include "sf_int.h"

#ifndef SF_BFFFO
int SFB_NAME(clzsi2)(uint32_t x);
#endif

int SFB_NAME(clzdi2)(uint64_t x)
{
    uint32_t hi = (uint32_t)(x >> 32);
#ifdef SF_BFFFO /* config/m68k: bfffo in this body, no call to __clzsi2 */
    return hi ? __builtin_clz(hi) : 32 + __builtin_clz((uint32_t)x);
#else
    return hi ? SFB_NAME(clzsi2)(hi) : 32 + SFB_NAME(clzsi2)((uint32_t)x);
#endif
}
