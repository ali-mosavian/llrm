// flags: -O2 -march=i486 | -O2 -march=i486 -m32
extern void report(long value);

/* Six values carried round the recursion, more than any target keeps across a call. */
static long mix(long a, long b, long c, long d, long e, int n)
{
    if (n == 0) return a + b + c + d + e;
    return mix(b + 1, c + a % 5, d ^ b, e + c % 3, a + d % 7, n - 1);
}

long bench_recmany(int n)
{
    return mix(1, 2, 3, 4, 5, n);
}

int main(void)
{
    report(bench_recmany(1500));
    return 0;
}
