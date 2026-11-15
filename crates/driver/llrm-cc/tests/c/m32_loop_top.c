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

/* x_cordic: an inner loop of two arms that join in its latch. */
static const long atn[12] = { 51472, 30386, 16055, 8150, 4091, 2047, 1024, 512, 256, 128, 64, 32 };
long cordic(int n)
{
    long sum = 0;
    for (int i = 0; i < n; ++i) {
        long x = 39797, y = 0, z = (long)(i * 300 - 30000);
        for (int k = 0; k < 12; ++k) {
            long xn, yn;
            if (z >= 0) { xn = x - (y >> k); yn = y + (x >> k); z -= atn[k]; } else { xn = x + (y >> k); yn = y - (x >> k); z += atn[k]; }
            x = xn; y = yn;
        }
        sum += x + y;
    }
    return sum;
}
