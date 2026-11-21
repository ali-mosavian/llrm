extern void report(long value);

static int M[32][32], T[32][32];
long bench_x_transpose(int n)
{
    int i, j, r; long sum = 0;
    for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) M[i][j] = i * 37 + j;
    for (r = 0; r < 8; ++r) { for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) T[j][i] = M[i][j] + r; for (i = 0; i < n; ++i) for (j = 0; j < n; ++j) M[i][j] = T[i][j]; }
    for (i = 0; i < n; ++i) sum += M[i][n - 1 - i];
    return sum;
}

int main(void)
{
    report(bench_x_transpose(32));
    return 0;
}
