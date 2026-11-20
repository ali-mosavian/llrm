void fill_i64(short *t, long k)
{
    for (long i = 0; i < 64; i++)
        t[i] = (i * 3 + k) & 255;
}

typedef long v2i64 __attribute__((vector_size(16)));
typedef long v4i64 __attribute__((vector_size(32)));

void mul_v2i64(v2i64 *r, const v2i64 *a, const v2i64 *b)
{
    *r = *a * *b;
}

void mul_v4i64(v4i64 *r, const v4i64 *a, const v4i64 *b)
{
    *r = *a * *b;
}

long dot_short(const short *a, const short *b)
{
    long s = 0;
    for (int i = 0; i < 48; i++)
        s += (long)a[i] * b[i];
    return s;
}
