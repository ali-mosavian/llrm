/* Width-generic add, multiply, divide and compare skeletons, included after sf_codec.inc by sf32.c and sf64.c. The includer defines
 * SF_QNAN, SF_PACK(sign, e, m) (round and pack), and, for the multiplication and the division, the kernels
 *   mul_core(sign, ea, eb, ma, mb) and div_core(sign, ea, eb, ma, mb): ma, mb normalised (implicit one set), ea, eb their biased exponents;
 * each returns the packed result. */
#define SF_QUIET (SF_HID >> 1)
/* Normalising a denormal: shift the mantissa left by s and take s off the exponent. A target whose shift count must sit in one register (x86: cl)
 * defines SF_NORM_SUB_FIRST, so the count is read last and need not outlive the other use of s. */
#ifdef SF_NORM_SUB_FIRST
#define SF_NORM_APPLY(m, e, s) do { (e) -= (s); (m) <<= (s); } while (0)
#else
#define SF_NORM_APPLY(m, e, s) do { (m) <<= (s); (e) -= (s); } while (0)
#endif
#define SF_CLZ_UNPACKED (SF_W - 1 - SF_MB) /* leading zeros of a mantissa with its implicit one set */

static SF_T SF_N(add_bits)(SF_T a, SF_T b)
{
    if ((a & ~SF_SIGN) < (b & ~SF_SIGN)) { SF_T t = a; a = b; b = t; }
    /* From here |a| >= |b|: a NaN operand is a, and an infinite a with a NaN b cannot occur. */
    if ((a << 1) >= (SF_INF << 1)) {
        if (SF_N(is_nan)(a)) return a | SF_QUIET;
        return ((a ^ b) & SF_SIGN) && (b << 1) == (SF_INF << 1) ? SF_QNAN : a;
    }
    if ((b << 1) == 0) return (a << 1) ? a : a & b;
    int ea, eb;
    SF_T ma = SF_N(unpack)(a, &ea) << 3, mb = SF_N(unpack)(b, &eb) << 3;
    if (ea - eb > SF_MB + 2) return a; /* b is under a quarter of a's last place; MB + 2 is the smallest gap that always rounds away (2^k - 1.5 * 2^(k-MB-2) rounds down) */
    mb = sf_shr_sticky(mb, ea - eb);
    SF_T sign = a & SF_SIGN, m;
    if ((a ^ b) & SF_SIGN) {
        m = ma - mb;
        if (!m) return 0;
        int s = sf_clz(m) - (SF_CLZ_UNPACKED - 3);
        if (s > ea - 1) s = ea - 1;
        m <<= s;
        ea -= s;
    } else {
        m = ma + mb;
        if (m & ((SF_T)1 << (SF_MB + 4))) { m = sf_shr_sticky(m, 1); ea++; }
    }
    return SF_PACK(sign, ea, m);
}

static SF_T SF_N(sub_bits)(SF_T a, SF_T b) { return SF_N(add_bits)(a, b ^ SF_SIGN); }

#ifndef SF_EXT_MULDIV /* a target that supplies multiplication and division elsewhere defines it */
#ifdef SF_SPECIALS_ONCE
/* One range test per operand finds zero, infinity and NaN together; the cases themselves are out of the common path. */
#define SF_FINITE(x) ((SF_T)((x) << 1) - 2 < (SF_INF << 1) - 2)

static SF_T SF_N(mul_special)(SF_T a, SF_T b)
{
    SF_T sign = (a ^ b) & SF_SIGN;
    if (SF_N(is_nan)(a)) return a | SF_QUIET;
    if (SF_N(is_nan)(b)) return b | SF_QUIET;
    if ((a << 1) == 0 || (b << 1) == 0) return ((a << 1) == (SF_INF << 1) || (b << 1) == (SF_INF << 1)) ? SF_QNAN : sign;
    return sign | SF_INF;
}

static SF_T SF_N(div_special)(SF_T a, SF_T b)
{
    SF_T sign = (a ^ b) & SF_SIGN;
    if (SF_N(is_nan)(a)) return a | SF_QUIET;
    if (SF_N(is_nan)(b)) return b | SF_QUIET;
    if ((a << 1) == (SF_INF << 1)) return (b << 1) == (SF_INF << 1) ? SF_QNAN : sign | SF_INF;
    if ((b << 1) == (SF_INF << 1)) return sign;
    if ((b << 1) == 0) return (a << 1) == 0 ? SF_QNAN : sign | SF_INF;
    return sign;
}

static SF_T SF_N(mul_bits)(SF_T a, SF_T b)
{
    if (!(SF_FINITE(a) & SF_FINITE(b))) return SF_N(mul_special)(a, b);
    SF_T sign = (a ^ b) & SF_SIGN;
    int ea, eb;
    SF_T ma = SF_N(unpack)(a, &ea), mb = SF_N(unpack)(b, &eb);
    if (!(ma & SF_HID)) { int s = sf_clz(ma) - SF_CLZ_UNPACKED; SF_NORM_APPLY(ma, ea, s); }
    if (!(mb & SF_HID)) { int s = sf_clz(mb) - SF_CLZ_UNPACKED; SF_NORM_APPLY(mb, eb, s); }
    return mul_core(sign, ea, eb, ma, mb);
}

static SF_T SF_N(div_bits)(SF_T a, SF_T b)
{
    if (!(SF_FINITE(a) & SF_FINITE(b))) return SF_N(div_special)(a, b);
    SF_T sign = (a ^ b) & SF_SIGN;
    int ea, eb;
    SF_T ma = SF_N(unpack)(a, &ea), mb = SF_N(unpack)(b, &eb);
    if (!(ma & SF_HID)) { int s = sf_clz(ma) - SF_CLZ_UNPACKED; SF_NORM_APPLY(ma, ea, s); }
    if (!(mb & SF_HID)) { int s = sf_clz(mb) - SF_CLZ_UNPACKED; SF_NORM_APPLY(mb, eb, s); }
    return div_core(sign, ea, eb, ma, mb);
}
#else
static SF_T SF_N(mul_bits)(SF_T a, SF_T b)
{
    SF_T sign = (a ^ b) & SF_SIGN;
    if (((a << 1) >= (SF_INF << 1)) | ((b << 1) >= (SF_INF << 1))) {
        if (SF_N(is_nan)(a)) return a | SF_QUIET;
        if (SF_N(is_nan)(b)) return b | SF_QUIET;
        if ((a << 1) == 0 || (b << 1) == 0) return SF_QNAN;
        return sign | SF_INF;
    }
    if ((a << 1) == 0 || (b << 1) == 0) return sign;
    int ea, eb;
    SF_T ma = SF_N(unpack)(a, &ea), mb = SF_N(unpack)(b, &eb);
    if (!(ma & SF_HID)) { int s = sf_clz(ma) - SF_CLZ_UNPACKED; SF_NORM_APPLY(ma, ea, s); }
    if (!(mb & SF_HID)) { int s = sf_clz(mb) - SF_CLZ_UNPACKED; SF_NORM_APPLY(mb, eb, s); }
    return mul_core(sign, ea, eb, ma, mb);
}

static SF_T SF_N(div_bits)(SF_T a, SF_T b)
{
    SF_T sign = (a ^ b) & SF_SIGN;
    if (((a << 1) >= (SF_INF << 1)) | ((b << 1) >= (SF_INF << 1))) {
        if (SF_N(is_nan)(a)) return a | SF_QUIET;
        if (SF_N(is_nan)(b)) return b | SF_QUIET;
        if ((a << 1) == (SF_INF << 1)) return (b << 1) == (SF_INF << 1) ? SF_QNAN : sign | SF_INF;
        return sign;
    }
    if ((b << 1) == 0) return (a << 1) == 0 ? SF_QNAN : sign | SF_INF;
    if ((a << 1) == 0) return sign;
    int ea, eb;
    SF_T ma = SF_N(unpack)(a, &ea), mb = SF_N(unpack)(b, &eb);
    if (!(ma & SF_HID)) { int s = sf_clz(ma) - SF_CLZ_UNPACKED; SF_NORM_APPLY(ma, ea, s); }
    if (!(mb & SF_HID)) { int s = sf_clz(mb) - SF_CLZ_UNPACKED; SF_NORM_APPLY(mb, eb, s); }
    return div_core(sign, ea, eb, ma, mb);
}
#endif
#endif

/* The order of two operands; `unordered` is what a NaN operand returns. */
static int SF_N(cmp_bits)(SF_T a, SF_T b, int unordered)
{
    if (SF_N(is_nan)(a) || SF_N(is_nan)(b)) return unordered;
    if (!((a | b) << 1)) return 0;
    SF_S x = (SF_S)a, y = (SF_S)b;
    if (x < 0) x = (SF_S)SF_SIGN - x;
    if (y < 0) y = (SF_S)SF_SIGN - y;
    return x < y ? -1 : x > y;
}
