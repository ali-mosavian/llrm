/* sys_parse_args's shape: a loop that passes a local's address and a static's to a call. */
extern short far pascal Get( short *out, char *name );
static char tag[] = "x";

short sum( short n )
{
    short i, v, t = 0;
    for ( i = 0; i < n; i++ ) {
        Get( &v, tag );
        t += v;
    }
    return t;
}
