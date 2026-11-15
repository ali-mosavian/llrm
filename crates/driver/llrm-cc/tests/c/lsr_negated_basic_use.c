/* queens' `safe` at -O1: `row - r` is the negation of `r - row`, one register for both. */
__attribute__((noinline)) int safe(const int *q, int row, int col)
{
    int r;

    for (r = 0; r < row; ++r) {
        int d = q[r] - col;
        if (d == 0 || d == row - r || d == r - row) return 0;
    }
    return 1;
}
int main(void) { int q[4] = {0, 2, 0, 0}; return safe(q, 2, 1); }
