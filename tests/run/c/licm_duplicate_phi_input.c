// flags: -O2 | -O3
// bar: none (a crash test: the compiler panicked at -O2 on this program)
/* Two vector loops in the body of an outer loop start from the same constant vector (0, 1, 2, 3). Machine LICM made the
   second one's load a duplicate of the first and renamed its readers, but not the loop phi that took it, which then
   read a value nothing made ("a unique value" in the coalescer). */
extern void report(long value);

static int F[64], G[64];

static void fillf(int n)
{
    for (int j = 0; j < n; j++)
        F[j] = j;
}

static long sum(void)
{
    long f = 0;

    for (int j = 0; j < 64; j++)
        f += F[j] * (j + 1);
    return f;
}

int main(void)
{
    int n, j;

    for (n = 0; n <= 40; n += 8) {
        for (j = 0; j < 64; j++)
            G[j] = j;
        fillf(n);
        report(sum() + G[n]);
    }
    return 0;
}
