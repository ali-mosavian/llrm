// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* Dot product of two 64-word vectors through pointers; the count is a parameter, as in examples/dot. */
extern void report(long value);

static short va[64], vb[64];

__attribute__((noinline)) long dot(const short *a, const short *b, short n)
{
    long total = 0;
    short i;

    for (i = 0; i < n; i++)
        total += (long)a[i] * b[i];
    return total;
}

long bench_dot(void)
{
    short i;

    for (i = 0; i < 64; i++) {
        va[i] = 3 * i - 50;
        vb[i] = 100 - 5 * i;
    }
    return dot(va, vb, 64);
}

int main(void)
{
    report(bench_dot());
    return 0;
}
