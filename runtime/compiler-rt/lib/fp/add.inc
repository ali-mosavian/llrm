/* Width-generic add and subtract on bit patterns; included after the width header (f32.h, f64.h). */
/* Decode for add_bits, which calls it twice. A target that finds the call smaller defines SF_UNPACK_OUTLINE (x86 m32 -Os: inlined into both call sites
 * the decode costs 21 B more; mul.inc and div.inc inline theirs, the smaller choice there). */
#ifdef SF_UNPACK_OUTLINE
static __attribute__((noinline)) SF_T SF_N(unpack_out)(SF_T a, int *e) { return SF_UNPACK(a, e); }
#define SF_UNPACK_ADD SF_N(unpack_out)
#else
#define SF_UNPACK_ADD SF_UNPACK
#endif

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
    SF_T ma = SF_UNPACK_ADD(a, &ea) << 3, mb = SF_UNPACK_ADD(b, &eb) << 3;
    if (ea - eb > SF_MB + 2) return a; /* b is under a quarter of a's last place; MB + 2 is the smallest gap that always rounds away (2^k - 1.5 * 2^(k-MB-2) rounds down) */
    mb = SF_SHR_STICKY_ADD(mb, ea - eb);
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
