// A nest that walks a table down its columns has its loops swapped at -O3 (gcc's -floop-interchange); one
// whose cells a later pass reads back across the swap, or whose inner count follows the outer, is kept.
// Each answer must be the nest's own.
extern void report(long value);
static unsigned long acc_;
#define report(v) (acc_ = acc_ * 31UL + (unsigned long)(v))


#define N 32

static short a[N][N], b[N][N], c[N][N];

static long checksum(short t[N][N])
{
    long sum = 0;
    int i, j;
    for (i = 0; i < N; ++i)
        for (j = 0; j < N; ++j)
            sum = (sum * 31 + t[i][j]) % 100003;
    return sum;
}

static void columns(short x)
{
    int i, j;
    for (j = 0; j < N; ++j)
        for (i = 0; i < N; ++i)
            a[i][j] = (short)(i * 7 + j * 3 + x);
}

static void reads_back(short x)
{
    int i, j;
    for (j = 0; j < N - 1; ++j)
        for (i = 1; i < N; ++i)
            b[i][j] = (short)(b[i - 1][j + 1] + i + x);
}

static void triangle(short x)
{
    int i, j;
    for (j = 0; j < N; ++j)
        for (i = 0; i < j; ++i)
            c[i][j] = (short)(c[i][j] + i * j + x);
}

static int run_main(void)
{
    int i, j;
    for (i = 0; i < N; ++i)
        for (j = 0; j < N; ++j)
            b[i][j] = (short)(i + 2 * j);
    columns(5);
    reads_back(3);
    triangle(2);
    report(checksum(a));
    report(checksum(b));
    report(checksum(c));
    return 0;
}

long bench_x_loop_interchange(int n)
{
    int k;
    acc_ = 0;
    for (k = 0; k < n; ++k) run_main();
    return (long)acc_;
}

#undef report
int main(void)
{
    report(bench_x_loop_interchange(1));
    return 0;
}
