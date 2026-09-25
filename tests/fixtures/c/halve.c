/* Signed division by powers of two: shellsort's gap and a fixed scale. */
int half( int n ) { return n / 2; }

int eighth( int n ) { return n / 8; }

int gaps( int n )
{
    int g, s = 0;
    for ( g = n; g > 0; g /= 2 ) s += g;
    return s;
}
