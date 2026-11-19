// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

/* The Nib program's twin with its semantics: the three arrays zero-initialised, as `= [[0] * 8] * 8` is. Same output as matmul.c; the
   reference compilers' bytes for the init are what Nib is judged against (bench.py TWINS). */
unsigned long bench_matmul(unsigned short seed)
{
    short a[8][8] = {{0}}, b[8][8] = {{0}};
    long c[8][8] = {{0}};
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

int main(void)
{
    report((long)bench_matmul(0));
    return 0;
}
