/* The x86 m32 configuration: a widening 32x32 multiply; division in four 32/32 hardware divides (SF_DIV_HW32: -O2 llrm 400 divs 213582 -> 144984 clk, -Os 235207 -> 168463,
 * gcc/clang faster too, and smaller than the Newton form); bsr for the leading-zero count (every call in sf32.c/sf32_conv.c has a non-zero argument; with 0 bsr
 * leaves its result undefined). clz alone: -Os add 2053 -> 1902 B image, 86784 -> 84995 clk. */
#ifndef SF_X86_M32_H
#define SF_X86_M32_H
#define SF_MUL32X32(a, b, h, l) do { uint64_t p_ = (uint64_t)(a) * (b); *(h) = (uint32_t)(p_ >> 32); *(l) = (uint32_t)p_; } while (0)
#define SF_DIV_HW32 1
#define SF_FIX_NAN_UNDEF 1 /* NaN to int is undefined in C and poison in IR, and compiler-rt/libgcc do not return 0 either: the check is bytes over the reference */
/* f64 div in two 28-bit digits, each one `div` (llrm.x86.udiv64by32: the quotient in the low half, the remainder in the high half; hi < d). Only llrm-cc knows the intrinsic. */
unsigned long long llrm_udiv64by32(unsigned hi, unsigned lo, unsigned d) __asm__("llrm.x86.udiv64by32") __attribute__((const));
#define SF_UDIV64BY32(hi, lo, d) ((uint32_t)llrm_udiv64by32((hi), (lo), (d)))
#define SF_DIV64_UDIV 1
#ifndef __OPTIMIZE_SIZE__
#define SF_SPECIALS_ONCE 1 /* -O2 llrm mul 79912 -> 76737 clk, div 144865 -> 143041, same bytes; at -Os it costs 29 B */
#endif
#ifdef __OPTIMIZE_SIZE__
#define SF_UNPACK_OUTLINE 1 /* -Os add reach 471 -> 450 B */
#endif
#define SF_NORM_SUB_FIRST 1 /* the shift count lives in cl: m32 mul -Os 102727 -> 101059 clk, -55 B in the -Os image */
#define SF_CLZ32(x) ((int)__builtin_clz(x))
#endif
