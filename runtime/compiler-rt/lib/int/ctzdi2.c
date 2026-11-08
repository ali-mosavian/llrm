/* __ctzdi2: trailing zeros of a non-zero 64-bit value, from the 32-bit halves. */
#include "sf_int.h"

int SFB_NAME(ctzsi2)(uint32_t x);

int SFB_NAME(ctzdi2)(uint64_t x)
{
    uint32_t lo = (uint32_t)x;
    return lo ? SFB_NAME(ctzsi2)(lo) : 32 + SFB_NAME(ctzsi2)((uint32_t)(x >> 32));
}
