// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

/* Two calls and nothing between them: lattice paths. */
static long paths(int n, int m)
{
    if (n == 0 || m == 0) return 1;
    return paths(n - 1, m) + paths(n, m - 1);
}

long bench_rectwo(int n)
{
    return paths(n, n);
}

int main(void)
{
    report(bench_rectwo(9));
    return 0;
}
