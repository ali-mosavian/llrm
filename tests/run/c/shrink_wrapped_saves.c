// flags: -O1 -march=i486 -m32 | -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv | -Os -march=i486 -m32 | -O0 -march=i486 -m32
/* A function that returns early before it calls keeps several registers across a call with stack
   arguments in the block it sets up late: the saves must not lie where the arguments are stored. */
extern void report(long value);
static long mix(long a, long b, long c, long d, long e, long f, long g)
{
    return a + 2 * b + 3 * c + 4 * d + 5 * e + 6 * f + 7 * g;
}
static long f(long n, long m, long k)
{
    long x, y, z;
    if (n == 0) return m;
    x = mix(n, m, k, 4, 5, 6, 7);
    y = mix(x, n, m, k, 1, 2, 3);
    z = mix(y, x, n, m, k, 0, 1);
    return x + y + z + n * m + k;
}
static long g(long n, long m, long k)
{
    if (n > 3) return n;
    return f(n, m, k) + g(n + 1, m + 1, k);
}
int main(void)
{
    report(f(0, 9, 3));
    report(f(2, 5, 1));
    report(g(0, 2, 7));
    return 0;
}
