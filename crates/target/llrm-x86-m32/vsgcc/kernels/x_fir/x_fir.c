extern void report(long value);

static short x[512], coef[16];
long bench_x_fir(int n)
{
    int i, k; long sum = 0; unsigned s = 17;
    for (i = 0; i < 16; ++i) coef[i] = (short)(i * 100 - 700);
    for (i = 0; i < n + 16; ++i) { s = s * 1103515245u + 12345u; x[i] = (short)((s >> 16) & 0xFFF); }
    for (i = 0; i < n; ++i) { long acc = 0; for (k = 0; k < 16; ++k) acc += (long)x[i + k] * coef[k]; sum += acc >> 8; }
    return sum;
}

int main(void)
{
    report(bench_x_fir(400));
    return 0;
}
