extern void report(long value);

typedef long double LD;

static LD a[48], b[48], c[48];

static LD horner(const LD *p, int n, LD x)
{
    LD r = 0;
    for (int i = n - 1; i >= 0; i--) r = r * x + p[i];
    return r;
}

static LD dot(const LD *p, const LD *q, int n)
{
    LD s = 0;
    for (int i = 0; i < n; i++) s += p[i] * q[i];
    return s;
}

static LD newton(LD v)
{
    LD x = v;
    for (int i = 0; i < 12; i++) x = (x + v / x) * 0.5L;
    return x;
}

static void axpy(LD f, LD *y, const LD *x, int n)
{
    for (int i = 0; i < n; i++) y[i] += f * x[i];
}

long bench_x_ldmath(int n)
{
    LD t = 0;
    for (int i = 0; i < n; i++) {
        a[i] = i * 0.5L + 1;
        b[i] = 1.0L / (i + 1);
        c[i] = i;
    }
    for (int r = 0; r < 8; r++) {
        axpy(0.25L, c, a, n);
        t += horner(a, n, 1.001L) + dot(a, b, n) + newton(t + 2) + ((a[1] * a[2] + 1.5L) * a[3] - 2.25L);
    }
    return (long)(t * 16);
}

int main(void)
{
    report(bench_x_ldmath(40));
    return 0;
}
