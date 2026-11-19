extern void report(long value);

static double v[64];

/* 16-bit sample clip: the limits are constants, the middle is the sample. */
static int clip(double x)
{
    return (int)(x > 32767.0 ? 32767.0 : (x < -32768.0 ? -32768.0 : x));
}

long bench_x_fclip(int n)
{
    long t = 0;
    for (int i = 0; i < n; i++) v[i] = ((i * 37) % 101 - 50) * 900.0;
    for (int r = 0; r < 40; r++)
        for (int i = 0; i < n; i++) t += clip(v[i] * (r + 1) / 8.0);
    return t;
}

int main(void)
{
    report(bench_x_fclip(64));
    return 0;
}
