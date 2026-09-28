/* Reduced from qcport: spill slots below a big local array need two-byte
   displacements. */
int use( int far *p, int v );

int spills( int a, int b, int c, int d )
{
    int buf[100];
    int x = use( buf, a );
    int y = use( buf, b );
    int z = use( buf, c );
    int w = use( buf, d );
    return use( buf, x + y ) + use( buf, z + w ) + x * y + z * w;
}
