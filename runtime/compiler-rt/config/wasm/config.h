/* wasm32 knobs (pass with -include). */
#define SF_MUL32X32(a, b, h, l) do { uint64_t p_ = (uint64_t)(a) * (b); *(h) = (uint32_t)(p_ >> 32); *(l) = (uint32_t)p_; } while (0) /* one i64.mul; the 16x16 partials were f128 mul 2528 -> 1460 B, f64 mul 1023 -> 751 */
#define SF_CLZ32(x) ((int)__builtin_clz(x)) /* one i32.clz; the generic ladder is ~40 B per use */
#define SF_CLZ64(x) ((int)__builtin_clzll(x)) /* one i64.clz */
#define SF_CONV_NOINLINE 1 /* -O2 inlines the f64 int-conversion helpers: float.c 287 -> 655 B */
