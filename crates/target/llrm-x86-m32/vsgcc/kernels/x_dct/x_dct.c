extern void report(long value);

static int blk[8][8];
static void dct8(int *d, int stride)
{
    int t0 = d[0] + d[7 * stride], t7 = d[0] - d[7 * stride], t1 = d[stride] + d[6 * stride], t6 = d[stride] - d[6 * stride];
    int t2 = d[2 * stride] + d[5 * stride], t5 = d[2 * stride] - d[5 * stride], t3 = d[3 * stride] + d[4 * stride], t4 = d[3 * stride] - d[4 * stride];
    int a = t0 + t3, b = t0 - t3, c = t1 + t2, e = t1 - t2;
    d[0] = a + c; d[4 * stride] = a - c;
    d[2 * stride] = (b * 5 + e * 2) >> 2; d[6 * stride] = (b * 2 - e * 5) >> 2;
    d[stride] = (t7 * 7 + t6 * 5 + t5 * 3 + t4) >> 3; d[3 * stride] = (t7 * 5 - t6 - t5 * 7 - t4 * 3) >> 3;
    d[5 * stride] = (t7 * 3 - t6 * 7 + t5 + t4 * 5) >> 3; d[7 * stride] = (t7 - t6 * 3 + t5 * 5 - t4 * 7) >> 3;
}
long bench_x_dct(int n)
{
    int k, i, j; long sum = 0;
    for (k = 0; k < n; ++k) {
        for (i = 0; i < 8; ++i) for (j = 0; j < 8; ++j) blk[i][j] = (i * 16 + j * 5 + k) & 255;
        for (i = 0; i < 8; ++i) dct8(&blk[i][0], 1);
        for (j = 0; j < 8; ++j) dct8(&blk[0][j], 8);
        for (i = 0; i < 8; ++i) sum += blk[i][i];
    }
    return sum;
}

int main(void)
{
    report(bench_x_dct(64));
    return 0;
}
