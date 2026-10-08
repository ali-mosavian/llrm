extern void report(long value);

long bench_x_collatz(int n)
{
    long best = 0; int i;
    for (i = 1; i <= n; ++i) {
        unsigned long x = (unsigned long)i; int steps = 0;
        while (x != 1) { x = (x & 1) ? 3 * x + 1 : x >> 1; ++steps; }
        if (steps > best) best = steps;
    }
    return best;
}

int main(void)
{
    report(bench_x_collatz(300));
    return 0;
}
