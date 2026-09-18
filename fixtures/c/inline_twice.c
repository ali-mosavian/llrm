/* Two calls expose whether the whole-module inliner can cost a tiny private leaf. */
static short increment( short value )
{
    return (short) ( value + 1 );
}

short inlineTwice( short value )
{
    return (short) ( increment( value ) + increment( value ) );
}
