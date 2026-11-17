void fill_i64(short *t, long k)
{
    for (long i = 0; i < 64; i++)
        t[i] = (i * 3 + k) & 255;
}
