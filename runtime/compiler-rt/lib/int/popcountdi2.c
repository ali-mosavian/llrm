/* __popcountdi2: set bits of a 64-bit value, from the 32-bit halves. */
#include "sf_int.h"

int SFB_NAME(popcountsi2)(uint32_t x);

int SFB_NAME(popcountdi2)(uint64_t x)
{
    return SFB_NAME(popcountsi2)((uint32_t)x) + SFB_NAME(popcountsi2)((uint32_t)(x >> 32));
}
