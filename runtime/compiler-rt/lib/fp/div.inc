/* Width-generic divide skeleton: needs div_core(sign, ea, eb, ma, mb), as mul.inc needs mul_core. */
#ifdef SF_USE_SPECIALS_DIV
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

static SF_T SF_N(div_bits)(SF_T a, SF_T b)
{
    if (!(SF_FINITE(a) & SF_FINITE(b))) return SF_N(div_special)(a, b);
    SF_T sign = (a ^ b) & SF_SIGN;
    int ea, eb;
    SF_T ma = SF_UNPACK(a, &ea), mb = SF_UNPACK(b, &eb);
    if (!(ma & SF_HID)) { int s = sf_clz(ma) - SF_CLZ_UNPACKED; SF_NORM_APPLY(ma, ea, s); }
    if (!(mb & SF_HID)) { int s = sf_clz(mb) - SF_CLZ_UNPACKED; SF_NORM_APPLY(mb, eb, s); }
    return div_core(sign, ea, eb, ma, mb);
}
#else
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
    SF_T ma = SF_UNPACK(a, &ea), mb = SF_UNPACK(b, &eb);
    if (!(ma & SF_HID)) { int s = sf_clz(ma) - SF_CLZ_UNPACKED; SF_NORM_APPLY(ma, ea, s); }
    if (!(mb & SF_HID)) { int s = sf_clz(mb) - SF_CLZ_UNPACKED; SF_NORM_APPLY(mb, eb, s); }
    return div_core(sign, ea, eb, ma, mb);
}
#endif
