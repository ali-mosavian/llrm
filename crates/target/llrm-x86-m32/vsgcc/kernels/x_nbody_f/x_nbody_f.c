extern void report(long value);

static float px[5], py[5], vx[5], vy[5];
long bench_x_nbody_f(int n)
{
    int i, j, s; float e = 0;
    for (i = 0; i < 5; ++i) { px[i] = (float)i; py[i] = (float)(i * i) * 0.5f; vx[i] = 0.1f * i; vy[i] = -0.05f * i; }
    for (s = 0; s < n; ++s) {
        for (i = 0; i < 5; ++i) for (j = 0; j < 5; ++j) if (i != j) {
            float dx = px[j] - px[i], dy = py[j] - py[i], d2 = dx * dx + dy * dy + 0.01f;
            vx[i] += dx / d2 * 0.01f; vy[i] += dy / d2 * 0.01f;
        }
        for (i = 0; i < 5; ++i) { px[i] += vx[i] * 0.1f; py[i] += vy[i] * 0.1f; }
    }
    for (i = 0; i < 5; ++i) e += vx[i] * vx[i] + vy[i] * vy[i];
    return (long)(e * 100000.0f);
}

int main(void)
{
    report(bench_x_nbody_f(60));
    return 0;
}
