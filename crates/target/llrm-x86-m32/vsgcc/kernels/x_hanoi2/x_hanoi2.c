extern void report(long value);

static long moves; static int peg[3][16], top[3];
static void mv(int n, int from, int to, int via)
{
    if (n == 0) return;
    mv(n - 1, from, via, to);
    peg[to][top[to]++] = peg[from][--top[from]]; ++moves;
    mv(n - 1, via, to, from);
}
long bench_x_hanoi2(int n)
{
    int i; for (i = 0; i < n; ++i) peg[0][top[0]++] = n - i;
    mv(n, 0, 2, 1);
    return moves * 100 + top[2];
}

int main(void)
{
    report(bench_x_hanoi2(12));
    return 0;
}
