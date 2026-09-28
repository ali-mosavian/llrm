/* Reduced from qcport's ls.c: ls_selftest's stores before ls_animate. */
typedef struct { const char *pattern; short length, frame, value; } Entry;
typedef struct { Entry tab[4]; float last; } Styles;

static short lchar( char c )
{
    return ( c - 'a' ) * 10;
}

void init( Styles *s )
{
    short i;
    for ( i = 0; i < 4; i++ ) {
        s->tab[i].pattern = "m";
        s->tab[i].length = 1;
        s->tab[i].frame = 0;
        s->tab[i].value = 0;
    }
    s->last = 0.0f;
}

void animate( Styles *s, float t )
{
    short i, steps, nf;

    steps = (short) ( (long) ( t * 10.0f ) - (long) ( s->last * 10.0f ) );
    if ( steps <= 0 ) return;
    s->last = t;
    for ( i = 0; i < 4; i++ ) {
        if ( s->tab[i].length > 1 ) {
            nf = (short) ( ( s->tab[i].frame + steps ) % s->tab[i].length );
            s->tab[i].frame = nf;
            s->tab[i].value = lchar( s->tab[i].pattern[nf] );
        }
    }
}

short test( void )
{
    Styles s;

    init( &s );
    animate( &s, 2.05f );
    s.tab[2].pattern = "az";
    s.tab[2].length = 2;
    s.tab[2].frame = 0;
    s.tab[2].value = lchar( 'a' );
    s.last = 0.0f;
    animate( &s, 0.1f );
    if ( s.tab[2].value != lchar( 'z' ) ) return -2;
    return 1;
}
