/* Two short locals carried across a call in a loop. */
extern short far pascal Map( short h );

short total( short n )
{
    short i, t = 0;
    for ( i = 0; i < n; i++ )
        t += Map( i );
    return t;
}
