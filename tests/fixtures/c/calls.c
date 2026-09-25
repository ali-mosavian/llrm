/* snd_fetch's shape: a local stored before a call and counted in a loop after it. */
short hnd;
extern short far pascal Map( short h, short page, short slot );

short fill( short n, signed char *out )
{
    short k = 0;
    if ( Map( hnd, 1, 2 ) == 0 ) return 0;
    while ( k < n ) {
        out[k] = 1;
        k++;
    }
    return n;
}
