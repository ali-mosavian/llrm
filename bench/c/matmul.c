/* Small enough for every DOS memory model, large enough to expose three loops. */
unsigned long bench_matmul(unsigned short seed)
{
    short a[8][8], b[8][8];
    long c[8][8];
    unsigned short i, j, k;
    unsigned long checksum = 0;

    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) {
            a[i][j] = (short)(i * 3 + j + 1 + seed);
            b[i][j] = (short)(i == j ? 2 : (i + j) % 3);
        }
    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) {
            long total = 0;
            for (k = 0; k < 8; ++k) total += (long)a[i][k] * b[k][j];
            c[i][j] = total;
        }
    for (i = 0; i < 8; ++i)
        for (j = 0; j < 8; ++j) checksum += (unsigned long)c[i][j] * (i * 8 + j + 1);
    return checksum;
}
