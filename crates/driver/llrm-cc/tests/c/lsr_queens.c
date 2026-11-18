/* queens at -Os: the row test of `safe`, inlined into `place`, kept a pointer, a count and a negated count
   where one index is smaller (the -Os object was 192 B, 169 B with the loop as written). */
static int safe(const int *q, int row, int col)
{
    for (int r = 0; r < row; ++r) {
        int d = q[r] - col;
        if (d == 0 || d == row - r || d == r - row) return 0;
    }
    return 1;
}

static int place(int *q, int row, int n)
{
    int found = 0;

    if (row == n) return 1;
    for (int col = 0; col < n; ++col)
        if (safe(q, row, col)) {
            q[row] = col;
            found += place(q, row + 1, n);
        }
    return found;
}

long solve(int n)
{
    int q[12];

    return place(q, 0, n);
}
