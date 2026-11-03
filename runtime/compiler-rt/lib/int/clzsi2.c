/* __clzsi2: leading zeros of a non-zero 32-bit value. Shared by the soft-float operations where an inline copy in each costs more than the call (SF_CLZ_CALL). */
#include "sf_int.h"

int SFB_NAME(clzsi2)(uint32_t x)
{
    int n = 0;
    if (!(x & 0xFFFF0000u)) { n += 16; x <<= 16; }
    if (!(x & 0xFF000000u)) { n += 8; x <<= 8; }
    if (!(x & 0xF0000000u)) { n += 4; x <<= 4; }
    if (!(x & 0xC0000000u)) { n += 2; x <<= 2; }
    if (!(x & 0x80000000u)) { n += 1; }
    return n;
}
