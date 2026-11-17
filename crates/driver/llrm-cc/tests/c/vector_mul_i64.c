void fill_i64(short *t, long k)
{
    for (long i = 0; i < 64; i++)
        t[i] = (i * 3 + k) & 255;
}

long dot_int(const int *a, const int *b, int n)
{
    long s = 0;
    for (int i = 0; i < n; i++)
        s += (long)a[i] * b[i];
    return s;
}
