/* __clzdi2: leading zeros of a non-zero 64-bit value, from the 32-bit halves. */
#include "sf_int.h"

int SFB_NAME(clzsi2)(uint32_t x);

int SFB_NAME(clzdi2)(uint64_t x)
{
    uint32_t hi = (uint32_t)(x >> 32);
    return hi ? SFB_NAME(clzsi2)(hi) : 32 + SFB_NAME(clzsi2)((uint32_t)x);
}
