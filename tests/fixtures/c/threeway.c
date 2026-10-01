/* strcmp's result is a three-way compare: its sign is no frequency. */
int strcmp( const char *a, const char *b );

int greater( const char *a, const char *b )
{
    return strcmp( a, b ) > 0;
}
