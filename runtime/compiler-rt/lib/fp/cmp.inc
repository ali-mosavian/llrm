/* The order of two operands: -1, 0, 1, and 1 when a NaN operand makes them unordered. Two arguments only, so an entry point that
 * passes its own arguments on is a tail jump. With SF_CMP_SPLIT (a target that links whole objects per entry) each entry is its own file and the core lives in the cmp entry alone (cmp_cmp.c); the others tail-jump or call it; gt and ge, which need the reversed compare, call it at -Os and carry their own copy otherwise (a far call costs more than the bytes it saves). */
#ifdef SF_CMP_SPLIT
#define SF_STR_(x) #x
#define SF_STR(x) SF_STR_(x)
/* The alias string is the target's mangled name: a regcall-default target (x86 m16) mangles to __regcall3__<name>, SF_ALIAS_PREFIX. */
#ifndef SF_ALIAS_PREFIX
#define SF_ALIAS_PREFIX ""
#endif
#define SF_CMP_ALIAS(target) __attribute__((alias(SF_ALIAS_PREFIX SF_STR(target))))
#endif
#if !defined(SF_CMP_SPLIT) || defined(SF_CMP_ONLY_CMP) || (defined(SF_CMP_ONLY_GE) && !defined(__OPTIMIZE_SIZE__))
static int SF_N(sf_cmp_bits)(SF_T a, SF_T b)
{
#if defined(SF_WIDE_ABS_MASK) && SF_W > 32 /* masks, not shifts, where a 64-bit shift is a double-word shift (shld+shl): m32 f64 cmp -O2 -12.7% clk, -23 B; m68k -O2 -112 B. Not for one word (m68k f32 +84 B) nor wasm (native i64 shift, +224 B) */
    if ((a & ~(SF_T)SF_SIGN) > SF_INF || (b & ~(SF_T)SF_SIGN) > SF_INF) return 1;
    if (!((a | b) & ~(SF_T)SF_SIGN)) return 0;
#else
    if (SF_N(is_nan)(a) || SF_N(is_nan)(b)) return 1;
    if (!((a | b) << 1)) return 0;
#endif
    SF_S x = (SF_S)a, y = (SF_S)b;
#ifdef SF_CMP_BRANCHY /* compiler-rt's form: same-sign operands compare as they are, else reversed, each side three-way on branches (no swap selects) */
    if ((x & y) >= 0) return x < y ? -1 : x != y;
    return x > y ? -1 : x != y;
#else
    if ((x & y) < 0) { SF_S t = x; x = y; y = t; } /* both negative: reversed order */
    if (x < y) return -1;
    if (x == y) return 0;
    return 1;
#endif
}
#endif
