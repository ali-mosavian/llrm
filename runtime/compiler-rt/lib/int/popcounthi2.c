/* __popcounthi2: set bits of a 16-bit value (no table, no multiply). */
#include "sf_int.h"

int SFB_NAME(popcounthi2)(uint16_t x)
{
    x = x - ((x >> 1) & 0x5555u);
    x = (x & 0x3333u) + ((x >> 2) & 0x3333u);
    x = (x + (x >> 4)) & 0x0F0Fu;
    return (x + (x >> 8)) & 0x1Fu;
}
