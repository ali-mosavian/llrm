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

long memalloc( long n );

/* A long result cast to a far pointer, as qcport's QGLMEMALLOC. */
void keep( Slot *s, long n )
{
    s->p = (char far *) memalloc( n );
    s->n = 1;
}

long rowptr( long surface, short y );

/* Reduced from qcport's d_sky.c: rows from long results, both dereferenced. */
void copyrows( long from, long to )
{
    short y, x;
    for ( y = 0; y < 4; y++ ) {
        unsigned char far *a = (unsigned char far *) rowptr( from, y );
        unsigned char far *b = (unsigned char far *) rowptr( to, y );
        for ( x = 0; x < 128; x++ ) if ( a[x] ) b[x + 128] = a[x + 128];
    }
}
