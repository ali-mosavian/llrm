/* The aarch64 configuration: native widening multiplies (the generic fallback is 16x16 partial products: f128 mul 2276 -> 1336 B at aarch64 -Os, 2912 -> 1246 at riscv64) and, for
 * aarch64, binary128 as the IEEE quad `long double` the tf libcalls take in vector registers. riscv64 passes the quad as an integer register pair, the bits ABI. */
#ifndef SF_AARCH64_H
#define SF_AARCH64_H
#include <stdint.h>
#define SF_MUL32X32(a, b, h, l) do { uint64_t p_ = (uint64_t)(a) * (b); *(h) = (uint32_t)(p_ >> 32); *(l) = (uint32_t)p_; } while (0)
#define SF_MUL64X64(a, b, h, l) do { unsigned __int128 p_ = (unsigned __int128)(a) * (b); *(h) = (uint64_t)(p_ >> 64); *(l) = (uint64_t)p_; } while (0)
#define SF_CLZ32(x) ((int)__builtin_clz(x))
#define SF_CLZ64(x) ((int)__builtin_clzll(x))
#define SF_QUAD_LONGDOUBLE 1
#define SF_CONV_SPLIT 1 /* f64 int-to-float one object per entry: a program converting only int32 linked all four (+484 B with pack) */
#define SF_CMP_SPLIT 1 /* compare entries alias one core in two objects, as compiler-rt's comparedf2: seven inlined copies were 532 B per width against its 216 */
#define SF_CMP_BRANCHY 1 /* compare core in compiler-rt's branchy form: __ltdf2 18.0 -> 15.7 executed insns on aarch64 (compiler-rt 16.0) */
#ifndef __OPTIMIZE_SIZE__ /* -O2: f64 div 643 / 701 -> about 125 / 139 executed instructions (compiler-rt 98 / 127), +124 / +136 B; -Os keeps the bit loop (strict bytes) */
#define SF_DIV_NEWTON 1
#endif
#endif
