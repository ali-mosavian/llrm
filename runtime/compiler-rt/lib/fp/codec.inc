/* Width-generic decode and encode of the soft-float core, included by sf32.c, sf32_conv.c and sf64.c.
 * The includer defines: SF_T (bit-pattern type), SF_W (its width), SF_MB (stored mantissa bits), SF_EXPMAX (all-ones exponent),
 * sf_clz, sf_shr_sticky and SF_ROUND3 for that width, and SF_N(name), the name of a static function (the same name for binary32, a suffixed one for the
 * wider type so that both fit one translation unit). A translation unit includes this once per width and ends with core_end.inc. */
#define SF_SIGN ((SF_T)1 << (SF_W - 1))
#define SF_HID ((SF_T)1 << SF_MB)
#define SF_INF ((SF_T)SF_EXPMAX << SF_MB)

static inline int SF_N(is_nan)(SF_T a) { return (a << 1) > (SF_INF << 1); }

/* Mantissa and exponent; denormals get e = 1, no implicit bit. */
static inline SF_T SF_N(unpack)(SF_T a, int *e)
{
    SF_T m = a & (SF_HID - 1);
    int x = (int)((a >> SF_MB) & SF_EXPMAX);
    if (x) m |= SF_HID; else x = 1;
    *e = x;
    return m;
}

/* Round to nearest even and pack: m has three guard bits and its implicit one at bit MB + 3; e may be <= 0 or >= SF_EXPMAX. */
static inline SF_T SF_N(pack_any)(SF_T sign, int e, SF_T m)
{
    if (e >= SF_EXPMAX) return sign | SF_INF;
    if (e <= 0) { m = sf_shr_sticky(m, 1 - e); e = 1; }
    m = SF_ROUND3(m);
    return sign + ((SF_T)(e - 1) << SF_MB) + m;
}
