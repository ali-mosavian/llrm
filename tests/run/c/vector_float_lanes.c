// flags: -O2 | -O3
// bar: none (a correctness test: float vector loops of every trip count)
/* Loops over double and float lanes the way programs write them: scale a table, fill it from an int counter, a
   multiply-add. The vector loop and its scalar remainder must agree at every trip count; a lane the vector code
   mishandles (a conversion that reads the wrong half, a store of the wrong width) changes the sum. */
extern void report(long value);

static double A[64], B[64];
static float F[64], G[64];

static void scale(int n) { for (int j = 0; j < n; j++) A[j] = B[j] * 0.5; }
static void fill(int n, int base) { for (int j = 0; j < n; j++) A[j] = (base + j) * 0.25; }
static void axpy(int n) { for (int j = 0; j < n; j++) F[j] = G[j] * 2.0f + 1.0f; }
static void fillf(int n) { for (int j = 0; j < n; j++) F[j] = j; }

static long sum(int n)
{
    double d = 0;
    float f = 0;
    for (int j = 0; j < 64; j++) {
        d += A[j] * (j + 1);
        f += F[j] * (j + 1);
    }
    return (long)(d * 4) + (long)(f * 4) + n;
}

int main(void)
{
    int n, j;

    for (n = 0; n <= 40; n++) {
        for (j = 0; j < 64; j++) {
            A[j] = -1;
            F[j] = -1;
            B[j] = j * 1.5 + 0.25;
            G[j] = j * 0.75f;
        }
        scale(n);
        report(sum(n));
        fill(n, n - 7);
        report(sum(n));
        axpy(n);
        report(sum(n));
        fillf(n);
        report(sum(n));
    }
    return 0;
}
