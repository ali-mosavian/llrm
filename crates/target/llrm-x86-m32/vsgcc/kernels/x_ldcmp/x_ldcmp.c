extern void report(long value);

typedef long double LD;

static LD v[64];

static LD maxof(const LD *p, int n)
{
    LD m = p[0];
    for (int i = 1; i < n; i++) if (p[i] > m) m = p[i];
    return m;
}

static LD clamp(LD x, LD lo, LD hi)
{
    return x < lo ? lo : (x > hi ? hi : x);
}

static int cmp3(LD a, LD b)
{
    return a < b ? -1 : (a > b);
}

static LD fabs_(LD x)
{
    return x < 0 ? -x : x;
}

long bench_x_ldcmp(int n)
{
    long t = 0;
    for (int i = 0; i < n; i++) v[i] = ((i * 37) % 23 - 11) * 0.75L;
    for (int r = 0; r < 20; r++) {
        for (int i = 0; i < n; i++) {
            t += cmp3(v[i], v[(i + r) % n]);
            t += (long)clamp(v[i], -3, 4) + (long)fabs_(v[i]);
        }
        t += (long)maxof(v, n);
    }
    return t;
}

int main(void)
{
    report(bench_x_ldcmp(50));
    return 0;
}
