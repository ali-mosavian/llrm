/* Reduced from qcport's sc.c: sc_selftest_run's link table. */
void set( short far *p, short v );
short get( short far *p );

short test( short far *p )
{
    short links[5] = { 0x101, 0x201, 0x401, 7, 1793 };
    short j;
    for ( j = 0; j < 5; j++ ) {
        set( p, links[j] );
        if ( get( p ) != links[j] ) return -109;
    }
    return 1;
}
