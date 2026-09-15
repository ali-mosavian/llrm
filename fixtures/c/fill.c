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

void use_shorts(short *p);

void fill_local(int n)
{
    short acc[8];
    int i;
    for (i = 0; i < n; i++) acc[i] = 0;
    use_shorts(acc);
}

static int far *paint;

void fill_through(int n)
{
    int k;
    for (k = 0; k < n; k++) paint[k] = 0;
}
