void words(int *restrict c, const short *restrict a, const short *restrict b, int n)
{
    for (int i = 0; i < n; ++i)
        c[i] = a[i] * b[i];
}

void unsigned_words(unsigned *restrict c, const unsigned short *restrict a, const unsigned short *restrict b, int n)
{
    for (int i = 0; i < n; ++i)
        c[i] = (unsigned)a[i] * b[i];
}

void bytes(int *restrict c, const signed char *restrict a, const signed char *restrict b, int n)
{
    for (int i = 0; i < n; ++i)
        c[i] = a[i] * b[i];
}

long dot(const short *a, const short *b)
{
    long total = 0;
    for (int i = 0; i < 48; ++i)
        total += (long)a[i] * b[i];
    return total;
}

void scaled(int *restrict c, const short *restrict a, int n)
{
    for (int i = 0; i < n; ++i)
        c[i] = a[i] * 1000;
}

short back[2000];

long weighted(void)
{
    long total = 0;
    for (long i = 0; i < 2000; ++i)
        total += (long)((int)back[i] * ((i & 15) + 1));
    return total;
}
