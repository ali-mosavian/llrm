/* Counted loops storing one value into consecutive cells: sieve's reset, and a word array to a parameter. */
static char flags[100];
static int words[50];

void fill_bytes( void )
{
    int i;
    for ( i = 0; i <= 99; i++ ) flags[i] = 1;
}

void fill_words( int n, int v )
{
    int i;
    for ( i = 0; i < n; i++ ) words[i] = v;
}

static short far reached[64];
static char far used[64];

void fill_far(int last, int n)
{
    int i;
    for (i = 0; i <= last; i++) reached[i] = 0;
    for (i = 0; i < n; i++) used[i] = 0;
}

int fill_counted(int n)
{
    int i;
    for (i = 0; i < n; i++) words[i] = 0;
    return i;
}
