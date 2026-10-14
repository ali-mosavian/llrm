// flags: -O0 | -O2 | -O3
// A loop of a run-time `n` trips over a 12 x 12 array is copied out by its extent at -O3 (each copy keeps its exit):
// every n from 0 to 12 must sum what the rolled loop does, the short ones leaving at their own test.
extern void report(long value);

static double A[12][12], B[12][12];

static double dot(int n, int i, int j)
{
    double sum = 0;
    int k;

    for (k = 0; k < n; k++)
        sum += A[i][k] * B[k][j];
    return sum;
}

int main(void)
{
    int i, j, n;

    for (i = 0; i < 12; i++)
        for (j = 0; j < 12; j++) {
            A[i][j] = (i + j) * 0.5;
            B[i][j] = (i - j) * 0.25 + 1.0;
        }
    for (n = 0; n <= 12; n++)
        report((long)(dot(n, 3, 5) * 16) + (long)(dot(n, 11, 0) * 16));
    return 0;
}
