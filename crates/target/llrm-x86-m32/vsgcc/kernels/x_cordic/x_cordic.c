extern void report(long value);

static const long atn[12] = { 51472, 30386, 16055, 8150, 4091, 2047, 1024, 512, 256, 128, 64, 32 };
long bench_x_cordic(int n)
{
    int i, k; long sum = 0;
    for (i = 0; i < n; ++i) {
        long x = 39797, y = 0, z = (long)(i * 300 - 30000);
        for (k = 0; k < 12; ++k) {
            long xn, yn;
            if (z >= 0) { xn = x - (y >> k); yn = y + (x >> k); z -= atn[k]; } else { xn = x + (y >> k); yn = y - (x >> k); z += atn[k]; }
            x = xn; y = yn;
        }
        sum += x + y;
    }
    return sum;
}

int main(void)
{
    report(bench_x_cordic(200));
    return 0;
}
