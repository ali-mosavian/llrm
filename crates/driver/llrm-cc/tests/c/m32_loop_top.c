typedef long double LD;
LD x[64], y[64];

/* Eight outer trips, each a dot product of n terms added to a running sum
 * (x_ld_dot). */
long dots(int n)
{
    LD t = 0;
    for (int r = 0; r < 8; r++) {
        LD s = 0;
        for (int i = 0; i < n; i++) s += x[i] * y[i];
        t += s;
        x[r] += 1;
    }
    return (long)(t * 0.5f);
}
