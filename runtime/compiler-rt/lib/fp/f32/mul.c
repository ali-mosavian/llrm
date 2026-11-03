/* __mulsf3. The kernel multiplies the normalised mantissas; a target that supplies multiplication elsewhere (6502) defines SF_EXT_MULDIV. */
#include "f32.h"

#ifndef SF_EXT_MULDIV
static inline sf32 mul_core(uint32_t sign, int ea, int eb, uint32_t ma, uint32_t mb)
{
    uint32_t hi, lo;
#ifdef SF_MUL_LOOP
    sf_mul24x24_loop(ma, mb, &hi, &lo);
#else
    sf_mul32x32(ma, mb, &hi, &lo);
#endif
    /* The product has its top bit at 47 or 46; keep 27 bits and fold the rest. */
    int e = ea + eb - 126;
    if (!(hi >> 15)) { e--; hi = (hi << 1) | (lo >> 31); lo <<= 1; }
    uint32_t m = (hi << 11) | (lo >> 21) | ((lo << 11) != 0);
    return pack_near(sign, e, m);
}

#include "../mul.inc"

sf_float SF_NAME(mulsf3)(sf_float a, sf_float b) { return b2f(mul_bits(f2b(a), f2b(b))); }
#endif

#include "../core_end.inc"
