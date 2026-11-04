/* Width-generic multiply skeleton: needs mul_core(sign, ea, eb, ma, mb) from the includer (ma, mb normalised with the implicit one set, ea, eb their biased exponents; returns the packed result). */
#ifdef SF_USE_SPECIALS_MUL
static SF_T SF_N(mul_special)(SF_T a, SF_T b)
{
    SF_T sign = (a ^ b) & SF_SIGN;
    if (SF_N(is_nan)(a)) return a | SF_QUIET;
    if (SF_N(is_nan)(b)) return b | SF_QUIET;
    if ((a << 1) == 0 || (b << 1) == 0) return ((a << 1) == (SF_INF << 1) || (b << 1) == (SF_INF << 1)) ? SF_QNAN : sign;
    return sign | SF_INF;
}

static SF_T SF_N(mul_bits)(SF_T a, SF_T b)
{
    if (!(SF_FINITE(a) & SF_FINITE(b))) return SF_N(mul_special)(a, b);
    SF_T sign = (a ^ b) & SF_SIGN;
    int ea, eb;
    SF_T ma = SF_UNPACK(a, &ea), mb = SF_UNPACK(b, &eb);
    if (!(ma & SF_HID)) { int s = sf_clz(ma) - SF_CLZ_UNPACKED; SF_NORM_APPLY(ma, ea, s); }
    if (!(mb & SF_HID)) { int s = sf_clz(mb) - SF_CLZ_UNPACKED; SF_NORM_APPLY(mb, eb, s); }
    return mul_core(sign, ea, eb, ma, mb);
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
    SF_T ma = SF_UNPACK(a, &ea), mb = SF_UNPACK(b, &eb);
    if (!(ma & SF_HID)) { int s = sf_clz(ma) - SF_CLZ_UNPACKED; SF_NORM_APPLY(ma, ea, s); }
    if (!(mb & SF_HID)) { int s = sf_clz(mb) - SF_CLZ_UNPACKED; SF_NORM_APPLY(mb, eb, s); }
    return mul_core(sign, ea, eb, ma, mb);
}

#endif
