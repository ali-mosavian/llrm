/* Reduced from qcport's sprite.c: a far result stored, then tested for null. */
void far *alloc( long n );
void fail( const char *why );

typedef struct { char far *p; short n; } Slot;

void load( Slot *s )
{
    s->p = (char far *) alloc( 168 );
    if ( !s->p ) fail( "none" );
}

char far *take( long n )
{
    char far *p = (char far *) alloc( n );
    if ( p == 0 ) fail( "none" );
    return p;
}
