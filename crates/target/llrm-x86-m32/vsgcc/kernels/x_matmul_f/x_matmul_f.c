extern void report(long value);

static double A[12][12], B[12][12], C[12][12];
long bench_x_matmul_f(int n)
{
    int i, j, k; double t = 0;
    for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) { A[i][j] = (i + j) * 0.5; B[i][j] = (i - j) * 0.25 + 1.0; }
    for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) { double s = 0; for (k = 0; k < n; ++k) s += A[i][k] * B[k][j]; C[i][j] = s; t += s; }
    return (long)t;
}

int main(void)
{
    report(bench_x_matmul_f(12));
    return 0;
}
