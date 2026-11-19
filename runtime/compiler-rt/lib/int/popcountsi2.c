/* __popcountsi2: set bits of a 32-bit value (no table, no multiply: the targets that call it have none or a slow one). */
#include "sf_int.h"

int SFB_NAME(popcountsi2)(uint32_t x)
{
    x = x - ((x >> 1) & 0x55555555u);
    x = (x & 0x33333333u) + ((x >> 2) & 0x33333333u);
    x = (x + (x >> 4)) & 0x0F0F0F0Fu;
    x += x >> 8;
    x += x >> 16;
    return x & 0x3Fu;
}
