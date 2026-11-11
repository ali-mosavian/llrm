// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Dot product of two arrays of different lengths, to the shorter, 40 times with the first element changed each time; Nib has no globals. */
extern void report(long value);

static long dot(const short *a, short na, const short *b, short nb)
{
    long total = 0;
    short n = na < nb ? na : nb;
    short i;

    for (i = 0; i < n; ++i)
        total += (long)a[i] * b[i];
    return total;
}

long bench_dot(void)
{
    short a[64], b[48];
    long sum = 0;
    short i, r;

    for (i = 0; i < 64; ++i)
        a[i] = i * 3 - 90;
    for (i = 0; i < 48; ++i)
        b[i] = 70 - i * 2;
    for (r = 0; r < 40; ++r) {
        a[0] = r;
        sum += dot(a, 64, b, 48);
    }
    return sum;
}

int main(void)
{
    report(bench_dot());
    return 0;
}
