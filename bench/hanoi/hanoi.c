// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

/* Moves taken to carry n discs from peg a to peg b by way of peg c. */
static int hanoi(int n, int a, int b, int c)
{
    if (n == 0) return 0;
    return hanoi(n - 1, a, c, b) + 1 + hanoi(n - 1, c, b, a);
}

long bench_hanoi(int n)
{
    return hanoi(n, 1, 3, 2);
}

int main(void)
{
    report(bench_hanoi(13));
    return 0;
}
