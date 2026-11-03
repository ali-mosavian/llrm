/* The x86 m32 configuration: a widening 32x32 multiply; division in four 32/32 hardware divides (SF_DIV_HW32: -O2 llrm 400 divs 213582 -> 144984 clk, -Os 235207 -> 168463,
 * gcc/clang faster too, and smaller than the Newton form); bsr for the leading-zero count (every call in sf32.c/sf32_conv.c has a non-zero argument; with 0 bsr
 * leaves its result undefined). clz alone: -Os add 2053 -> 1902 B image, 86784 -> 84995 clk. */
#ifndef SF_X86_M32_H
#define SF_X86_M32_H
#define SF_MUL32X32(a, b, h, l) do { uint64_t p_ = (uint64_t)(a) * (b); *(h) = (uint32_t)(p_ >> 32); *(l) = (uint32_t)p_; } while (0)
#define SF_DIV_HW32 1
#define SF_CLZ32(x) ((int)__builtin_clz(x))
#endif
