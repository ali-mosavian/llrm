/* The z80 configuration. Include with -include. */
#ifndef SF_Z80_H
#define SF_Z80_H
#include <stdint.h>
/* One shared __clzsi2 (lib/int/clzsi2.c): llrm-z80-llc emits the inline clz as a global, so each module carried its own 204..216 B copy (mul -212 B, div -212, add_sub -203, float -167 at -Os). */
#define SF_CLZ_CALL 1
#define SF_SHIFT_LOOP 1 /* shift by a per-bit loop: fix_si keeps the two-direction shift (single larger count: 6502 -Os +15% cycles) */
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
/* The 57-step restoring loop of the f64 divide as a Z80 routine over fixed cells (config/z80/div64.s): the compiled loop costs ~3.0k clk per step. */
#define SF_DIV64_Z80 1
/* The 64x64 -> 128 of the f64 multiply as a Z80 routine over fixed cells (config/z80/mul64.s): the compiled form spills every operand and result word. */
extern uint64_t sf_m64_a, sf_m64_b, sf_m64_lo, sf_m64_hi;
void __sf_mul64(void);
static inline void sf_z80_mul64(uint64_t a, uint64_t b, uint64_t *hi, uint64_t *lo)
{
    sf_m64_a = a;
    sf_m64_b = b;
    __sf_mul64();
    *hi = sf_m64_hi;
    *lo = sf_m64_lo;
}
#define SF_MUL64X64 sf_z80_mul64
/* sdld links an object whole: one object per f64 compare group (__cmpdf2 with eq/ne/lt/le as aliases of it, __gedf2 with gt) and per int-to-float entry, so a program linking one links one. */
#define SF_CMP_SPLIT 1
/* the f64 decode is one shared object: add, mul and div each carried a copy (208 B) */
#define SF_UNPACK_SHARED 1
#define SF_CMP_NOALIAS 1 /* llrm-z80-llc emits no alias: eq/ne/lt/le jump to __cmpdf2, gt to __gedf2, each its own object */
#define SF_CONV_SPLIT 1
#endif
