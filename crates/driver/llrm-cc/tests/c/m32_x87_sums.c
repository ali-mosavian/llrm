typedef long double LD;

static LD x[64];

static LD horner(const LD *c, int n, LD x0)
{
    LD r = 0;
    for (int i = n - 1; i >= 0; i--) r = r * x0 + c[i];
    return r;
}

/* Eight loops, each result added to the running sum (x_ld_horner). */
long sums(int n)
{
    LD t = 0;
    for (int r = 0; r < 8; r++) t += horner(x, n, 1.0005L + r * 0.0001L);
    return (long)t;
}
