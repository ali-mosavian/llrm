/* __clzhi2: leading zeros of a non-zero 16-bit value (libgcc's name; the width of the targets whose register is 8 bits). */
#include "sf_int.h"

int SFB_NAME(clzhi2)(uint16_t x)
{
    int n = 0;
    if (!(x & 0xFF00u)) { n += 8; x <<= 8; }
    if (!(x & 0xF000u)) { n += 4; x <<= 4; }
    if (!(x & 0xC000u)) { n += 2; x <<= 2; }
    if (!(x & 0x8000u)) { n += 1; }
    return n;
}
