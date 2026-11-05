/* wasm32 knobs (pass with -include). */
#define SF_CLZ32(x) ((int)__builtin_clz(x)) /* one i32.clz; the generic ladder is ~40 B per use */
