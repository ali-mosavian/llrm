/* The order of two operands: -1, 0, 1, and 1 when a NaN operand makes them unordered. Two arguments only, so an entry point that
 * passes its own arguments on is a tail jump. */
static int SF_N(cmp_bits)(SF_T a, SF_T b)
{
    if (SF_N(is_nan)(a) || SF_N(is_nan)(b)) return 1;
    if (!((a | b) << 1)) return 0;
    SF_S x = (SF_S)a, y = (SF_S)b;
    if (x < 0) x = (SF_S)SF_SIGN - x;
    if (y < 0) y = (SF_S)SF_SIGN - y;
    return x < y ? -1 : x > y;
}
