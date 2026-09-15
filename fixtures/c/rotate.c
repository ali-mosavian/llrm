/* matmul's inner loop, bounded by a constant, and a loop bounded by a parameter. */
static int ma[20], mb[20];

int dot( void )
{
    int k, s = 0;
    for ( k = 0; k < 20; k++ ) s += ma[k] * mb[k];
    return s;
}

int fill( int n )
{
    int k;
    for ( k = 0; k < n; k++ ) ma[k] = k;
    return n;
}
