/* The x86 m16 (386+, no x87) configuration: a widening 32x32 multiply, Newton division, sf32_pack's common case inlined.
 * At -O2 each op beats or ties compiler-rt on clocks, at -Os on bytes (tests/x86_m16_kernels.sh); on the default 16x16 partial products the Newton division loses. */
#ifndef SF_X86_M16_H
#define SF_X86_M16_H
#define SF_MUL32X32(a, b, h, l) do { uint64_t p_ = (uint64_t)(a) * (b); *(h) = (uint32_t)(p_ >> 32); *(l) = (uint32_t)p_; } while (0)
#define SF_DIV_NEWTON 1
#ifndef __OPTIMIZE_SIZE__
#define SF_PACK_INLINE 1 /* bytes first at -Os: the call stays */
#endif
#endif
