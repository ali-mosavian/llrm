// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Arrays are 0-based, index i stands for BASIC's i-50 (C has no lower bounds). */
extern void report(long value);

static short px[101], py[101], pz[101];
static short vx[101], vy[101], vz[101];

static void advance(short n)
{
    short i;

    for (i = 50 - n; i <= 50 + n; ++i) {
        px[i] = px[i] + vx[i];
        py[i] = py[i] + vy[i];
        pz[i] = pz[i] + vz[i];
    }
}

long bench_particle(void)
{
    short i, t, p;
    long s;

    for (i = 0; i <= 100; ++i) {
        p = i - 50;
        px[i] = p; py[i] = 2 * p; pz[i] = -p;
        vx[i] = p & 7; vy[i] = 3 - (p & 3); vz[i] = 1;
    }
    for (t = 1; t <= 100; ++t)
        advance(50);
    s = 0;
    for (i = 0; i <= 100; ++i)
        s = s + px[i] + 3L * py[i] + 7L * pz[i];
    return s;
}

int main(void)
{
    report(bench_particle());
    return 0;
}
