/* __ctzdi2: trailing zeros of a non-zero 64-bit value, from the 32-bit halves. */
#include "sf_int.h"

#ifndef SF_BFFFO
int SFB_NAME(ctzsi2)(uint32_t x);
#endif

int SFB_NAME(ctzdi2)(uint64_t x)
{
    uint32_t lo = (uint32_t)x;
#ifdef SF_BFFFO /* config/m68k: bfffo in this body, no call to __ctzsi2 */
    return lo ? __builtin_ctz(lo) : 32 + __builtin_ctz((uint32_t)(x >> 32));
#else
    return lo ? SFB_NAME(ctzsi2)(lo) : 32 + SFB_NAME(ctzsi2)((uint32_t)(x >> 32));
#endif
}
