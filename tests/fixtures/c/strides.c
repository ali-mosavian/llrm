/* Frame arrays walked by one counter at different strides: no single
   16-bit index serves them all. */
long bench_strides3(unsigned short n)
{
    long l[16]; int w[16]; char b[16];
    unsigned short i, k;

    for (i = 0; i < 16; ++i) { l[i] = (long)i * n; w[i] = i + n; b[i] = (char)(i ^ n); }
    for (k = 0; k < n; ++k)
        for (i = 0; i < 16; ++i) {
            l[i] += w[i] + b[i];
            w[i] ^= (int)l[i];
            b[i] += (char)w[i];
        }
    return l[3] + w[5] + b[7];
}

long bench_strides4(unsigned short n)
{
    double d[16]; long l[16]; int w[16]; char b[16];
    unsigned short i, k;

    for (i = 0; i < 16; ++i) { d[i] = i * 0.5; l[i] = (long)i * n; w[i] = i + n; b[i] = (char)(i ^ n); }
    for (k = 0; k < n; ++k)
        for (i = 0; i < 16; ++i) {
            l[i] += w[i] + b[i];
            w[i] ^= (int)l[i];
            b[i] += (char)w[i];
            d[i] += (double)w[i];
        }
    return l[3] + w[5] + b[7] + (long)d[9];
}
