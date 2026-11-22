extern void report(long value);

__attribute__((noinline)) static long deep(long n, long a, long b)
{
    if (n == 0)
        return a + b;
    return deep(n - 1, b, a + n) + n;
}

int main(void)
{
    report(deep(5, 1, 2));
    return 0;
}
