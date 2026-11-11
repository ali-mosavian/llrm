/* __ctzhi2: trailing zeros of a non-zero 16-bit value. */
#include "sf_int.h"

int SFB_NAME(ctzhi2)(uint16_t x)
{
    int n = 0;
    if (!(x & 0x00FFu)) { n += 8; x >>= 8; }
    if (!(x & 0x000Fu)) { n += 4; x >>= 4; }
    if (!(x & 0x0003u)) { n += 2; x >>= 2; }
    if (!(x & 0x0001u)) { n += 1; }
    return n;
}
