// flags: -O0 | -O3 | -O3 -m32
// A nest that walks a table down its columns has its loops swapped at -O3 (gcc's -floop-interchange); one
// whose cells a later pass reads back across the swap, or whose inner count follows the outer, is kept.
// Each answer must be the nest's own.
extern void report(long value);

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

int main(void)
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
