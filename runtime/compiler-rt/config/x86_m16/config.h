/* The x86 m16 (386+, no x87) configuration: a widening 32x32 multiply, four hardware divides for the division, sf32_pack's common case inlined.
 * At -O2 each op beats or ties compiler-rt on clocks, at -Os on bytes (tests/x86_m16_kernels.sh). */
#ifndef SF_X86_M16_H
#define SF_X86_M16_H
#define SF_MUL32X32(a, b, h, l) do { uint64_t p_ = (uint64_t)(a) * (b); *(h) = (uint32_t)(p_ >> 32); *(l) = (uint32_t)p_; } while (0)
#define SF_DIV_HW32 1
#ifdef __OPTIMIZE_SIZE__
#define SF_CLZ_CALL 1 /* the 5-step ladder in every operation costs more than the call */
#else
#define SF_PACK_INLINE 1 /* bytes first at -Os: the call stays */
#define SF_SPECIALS_MUL 1 /* -O2 mul -4.7% clk for +1 B; with the division too +8 B and slower */
#endif
#endif
