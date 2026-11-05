/* The z80 configuration. Include with -include. */
#ifndef SF_Z80_H
#define SF_Z80_H
#include <stdint.h>
/* 32x32 -> 64 from four 16x16 -> 32 products (__umulhisi3, config/z80/int32.s): the generic form calls __mulsi3 four times per 32x32, each a full
 * 32-bit product whose cross terms are zero. */
uint32_t __umulhisi3(uint16_t a, uint16_t b);
static inline void sf_z80_mul32x32(uint32_t a, uint32_t b, uint32_t *hi, uint32_t *lo)
{
    uint16_t al = (uint16_t)a, ah = (uint16_t)(a >> 16), bl = (uint16_t)b, bh = (uint16_t)(b >> 16);
    uint32_t ll = __umulhisi3(al, bl), lh = __umulhisi3(al, bh), hl = __umulhisi3(ah, bl), hh = __umulhisi3(ah, bh);
    uint32_t mid = (ll >> 16) + (lh & 0xFFFFu) + (hl & 0xFFFFu);
    *lo = (ll & 0xFFFFu) | (mid << 16);
    *hi = hh + (lh >> 16) + (hl >> 16) + (mid >> 16);
}
#define SF_MUL32X32 sf_z80_mul32x32
#endif
