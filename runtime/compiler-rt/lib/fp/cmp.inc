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
