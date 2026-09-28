/* Two float-to-int conversions in a row: one switch of the rounding mode. */
void truncs( float a, float b, short *p )
{
    p[0] = (short) a;
    p[1] = (short) b;
}
