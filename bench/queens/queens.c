// flags: -O2 --cpu 486 | -O2 --cpu 486 --target x86-code32
extern void report(long value);

/* Row r holds a queen in column q[r]; row `row` is placed next. */
static int safe(const int *q, int row, int col)
{
    int r;

    for (r = 0; r < row; ++r) {
        int d = q[r] - col;
        if (d == 0 || d == row - r || d == r - row) return 0;
    }
    return 1;
}

static int place(int *q, int row, int n)
{
    int col, found = 0;

    if (row == n) return 1;
    for (col = 0; col < n; ++col)
        if (safe(q, row, col)) {
            q[row] = col;
            found += place(q, row + 1, n);
        }
    return found;
}

long bench_queens(int n)
{
    int q[12];

    return place(q, 0, n);
}

int main(void)
{
    report(bench_queens(7));
    return 0;
}
