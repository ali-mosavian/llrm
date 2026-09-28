/* Both arms of an if/else before the return. llrm left one arm after it,
   so its branch and its jump back were both long in a large function. */
int use( int v );

int diamond( int a, int b )
{
    int x;
    if ( a )
        x = use( b );
    else
        x = b + 7;
    return use( x );
}
