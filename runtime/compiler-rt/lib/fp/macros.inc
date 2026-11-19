/* Width-generic constants and knobs of the soft-float core; a width header (f32.h, f64.h) includes this in every file, and core_end.inc undefines it again.
 * The includer defines SF_T (bit-pattern type), SF_S (its signed twin), SF_W (width), SF_MB (stored mantissa bits), SF_EXPMAX (all-ones exponent), SF_QNAN,
 * and the width helpers sf_clz, sf_shr_sticky, SF_ROUND3, SF_PACK(sign, e, m). */
#define SF_SIGN ((SF_T)1 << (SF_W - 1))
#define SF_HID ((SF_T)1 << SF_MB)
#define SF_INF ((SF_T)SF_EXPMAX << SF_MB)
#define SF_QUIET (SF_HID >> 1)
/* Normalising a denormal: shift the mantissa left by s and take s off the exponent. A target whose shift count must sit in one register (x86: cl)
 * defines SF_NORM_SUB_FIRST, so the count is read last and need not outlive the other use of s. */
#ifdef SF_NORM_SUB_FIRST
#define SF_NORM_APPLY(m, e, s) do { (e) -= (s); (m) <<= (s); } while (0)
#else
#define SF_NORM_APPLY(m, e, s) do { (m) <<= (s); (e) -= (s); } while (0)
#endif
#define SF_CLZ_UNPACKED (SF_W - 1 - SF_MB) /* leading zeros of a mantissa with its implicit one set */

/* The config knobs SF_SPECIALS_ONCE (both) and SF_SPECIALS_MUL / SF_SPECIALS_DIV take the special operands out of the common path; SF_USE_* is the
 * per-file copy that core_end.inc may undefine without touching the config. */
#if defined(SF_SPECIALS_ONCE) || defined(SF_SPECIALS_MUL)
#define SF_USE_SPECIALS_MUL
#endif
#if defined(SF_SPECIALS_ONCE) || defined(SF_SPECIALS_DIV)
#define SF_USE_SPECIALS_DIV
#endif
#if defined(SF_USE_SPECIALS_MUL) || defined(SF_USE_SPECIALS_DIV)
/* One range test per operand finds zero, infinity and NaN together; the cases themselves are out of the common path. */
#define SF_FINITE(x) ((SF_T)((x) << 1) - 2 < (SF_INF << 1) - 2)
#endif
