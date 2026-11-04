/* The x86 m16 (386+, no x87) configuration: a widening 32x32 multiply, four hardware divides for the division; sf32_pack stays a call (inlining it was +154 B for 1% clocks: 3 -O2 cells over main).
 * At -O2 each op beats or ties compiler-rt on clocks, at -Os on bytes (tests/x86_m16_kernels.sh). */
#ifndef SF_X86_M16_H
#define SF_X86_M16_H
#define SF_MUL32X32(a, b, h, l) do { uint64_t p_ = (uint64_t)(a) * (b); *(h) = (uint32_t)(p_ >> 32); *(l) = (uint32_t)p_; } while (0)
#define SF_DIV_HW32 1
#define SF_ALIAS_PREFIX "__regcall3__" /* clang mangles regcall-default functions; an alias names the mangled symbol */
#define SF_FIX_NAN_UNDEF 1 /* NaN to int is undefined in C and poison in IR: no caller can rely on 0; the check costs +16/+9 B over compiler-rt */
/* SF_CMP_SPLIT (one file per compare entry, the core in __cmpsf2) is off: __ltsf2 reach 212 -> 111 B and all seven entries 172, but each compare gains a far jmp: +10 clk, +2 instr (Os kernel +7.4%). On when the entries can alias __cmpsf2 (OMF alias). */
#ifdef __OPTIMIZE_SIZE__
#define SF_CLZ_CALL 1 /* the 5-step ladder in every operation costs more than the call */
#else
#define SF_SPECIALS_MUL 1 /* -O2 mul -4.7% clk for +1 B; with the division too +8 B and slower */
#endif
#endif
