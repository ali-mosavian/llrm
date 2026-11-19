extern void report(long value);

typedef long double LD;

static LD x[64], y[64], z[64];

static void init(void)
{
    for (int i = 0; i < 64; i++) {
        x[i] = i * 0.5L + 1;
        y[i] = 1.0L / (i + 1);
        z[i] = i;
    }
}

static LD A[16][16], B[16][16], C[16][16];

static LD ld_matmul(int n)
{
    LD t = 0;
    for (int i = 0; i < n; i++) for (int j = 0; j < n; j++) { A[i][j] = (i + j) * 0.5L; B[i][j] = (i - j) * 0.25L + 1; }
    for (int i = 0; i < n; i++) for (int j = 0; j < n; j++) {
        LD s = 0;
        for (int k = 0; k < n; k++) s += A[i][k] * B[k][j];
        C[i][j] = s;
        t += s;
    }
    return t;
}

long bench_x_ld_matmul(int n)
{
    return (long)ld_matmul(n);
}

static void setup(void)
{
}

int main(void)
{
    setup();
    report(bench_x_ld_matmul(12));
    return 0;
}
