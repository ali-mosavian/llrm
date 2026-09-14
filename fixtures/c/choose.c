static short pick( short c, short a, short b )
{
    return c ? a : b;
}

static char near *which( short c, char near *a, char near *b )
{
    return c > 3 ? a : b;
}

short choose( short c )
{
    return pick( c, 7, 9 ) + ( which( c, "ab", "c" ) != 0 );
}
