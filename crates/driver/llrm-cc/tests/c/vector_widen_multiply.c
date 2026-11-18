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
