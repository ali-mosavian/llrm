/* __divsf3. The kernel divides the normalised mantissas; SF_DIV_NEWTON, SF_DIV_HW32 or one quotient bit per step, see README. */
#include "f32.h"
/* Open compiler issue (6502 backend is branch-polarity sensitive; owner llrm-target-6502, plan 11-07): the 6502 -Os defaults build lays out __divsf3 differently when the TU has another caller of
 * sf_clz32 (+0.7% cycles); repro ~/scratch/handoff/divsf3-6502/repro. The config/6502 build is unaffected. */

#ifndef SF_EXT_MULDIV
static inline sf32 div_core(uint32_t sign, int ea, int eb, uint32_t ma, uint32_t mb)
{
    /* q = floor(ma * 2^27 / mb) and whether it was exact. */
    uint32_t q;
    int inexact;
#ifdef SF_DIV_NEWTON
    /* Reciprocal x ~ 2^55 / mb by Newton-Raphson from the linear estimate 3/4 + 1/sqrt 2 - b/2 (compiler-rt's start), three
     * iterations; then q ~ ma * x / 2^28, off by at most 1 for every mb (checked exhaustively), and the remainder, which
     * only needs its low 32 bits, corrects it. mb = 2^23 makes x wrap: it stands for 2^32, so clamp. Eight 32x32 products per
     * call: with a widening SF_MUL32X32, x86 m16 400 divs 404k -> 277k clocks, x86 m32 through llrm-cc 280k -> 222k, but gcc 191k -> 223k
     * and with the default 16x16 partials m16 563k: an integrator opts in. */
    uint32_t hi, lo, mb8 = mb << 8, x = 0x7504F333u - mb8;
#define NEWTON_STEP() do { \
        sf_mul32x32(x, mb8, &hi, &lo); \
        uint32_t corr = 0u - hi; \
        sf_mul32x32(x, corr, &hi, &lo); \
        x = (hi << 1) | (lo >> 31); \
    } while (0)
    NEWTON_STEP(); NEWTON_STEP(); NEWTON_STEP();
    if (x < 0x80000000u) x = 0xFFFFFFFFu;
    sf_mul32x32(ma << 8, x, &hi, &lo);
    q = hi >> 4;
    uint32_t r = (ma << 27) - q * mb;
    while ((int32_t)r < 0) { q--; r += mb; }
    while (r >= mb) { q++; r -= mb; }
    inexact = r != 0;
#elif defined(SF_DIV_HW32)
    /* The quotient in four 32/32 hardware divisions: the integer bit, then 8, 8, 8 and 3 fraction bits, each a division of the remainder
     * (< mb < 2^24) shifted up by the step width. For a target whose 32-bit divide is cheap (x86 m32, m68k 020+). */
    uint32_t r = ma, t;
    q = r >= mb;
    if (q) r -= mb;
#define DIV_STEP(w) do { t = r << (w); q = (q << (w)) | t / mb; r = t % mb; } while (0)
    DIV_STEP(8); DIV_STEP(8); DIV_STEP(8); DIV_STEP(3);
    inexact = r != 0;
#else
    /* One quotient bit per step. */
    uint32_t r = ma;
    q = 0;
    for (int i = 0; i < 28; i++) {
        q <<= 1;
        if (r >= mb) { r -= mb; q |= 1; }
        r <<= 1;
    }
    inexact = r != 0;
#endif
    int e = ea - eb + 126;
    if (q >> 27) { q = (q >> 1) | (q & 1); e++; }
    return pack_near(sign, e, q | inexact);
}

#include "../div.inc"

sf_float SF_NAME(divsf3)(sf_float a, sf_float b) { return b2f(div_bits(f2b(a), f2b(b))); }
#endif

#include "../core_end.inc"
