/* The riscv64 configuration: native widening multiplies (the generic fallback is 16x16 partial products: f128 mul 2276 -> 1336 B at aarch64 -Os, 2912 -> 1246 at riscv64) and, for
 * aarch64, binary128 as the IEEE quad `long double` the tf libcalls take in vector registers. riscv64 passes the quad as an integer register pair, the bits ABI. */
#ifndef SF_RISCV64_H
#define SF_RISCV64_H
#include <stdint.h>
#define SF_MUL32X32(a, b, h, l) do { uint64_t p_ = (uint64_t)(a) * (b); *(h) = (uint32_t)(p_ >> 32); *(l) = (uint32_t)p_; } while (0)
#define SF_MUL64X64(a, b, h, l) do { unsigned __int128 p_ = (unsigned __int128)(a) * (b); *(h) = (uint64_t)(p_ >> 64); *(l) = (uint64_t)p_; } while (0)
#define SF_CLZ32(x) ((int)__builtin_clz(x))
#define SF_CLZ64(x) ((int)__builtin_clzll(x))
#endif
