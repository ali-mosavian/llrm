/* __ctzsi2: trailing zeros of a non-zero 32-bit value, from the leading-zero count of its lowest set bit (one ladder shared with __clzsi2). */
#include "sf_int.h"

#ifndef SF_BFFFO
int SFB_NAME(clzsi2)(uint32_t x);
#endif

int SFB_NAME(ctzsi2)(uint32_t x)
{
#ifdef SF_BFFFO /* config/m68k: 31 - bfffo(x & -x) in this body */
    return __builtin_ctz(x);
#else
    return 31 - SFB_NAME(clzsi2)(x & (0u - x));
#endif
}
