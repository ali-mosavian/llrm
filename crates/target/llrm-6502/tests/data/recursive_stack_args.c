extern void report(long value);
static long deep(long n, long a, long b, long c, long d)
{
    long t, s;
    if (n == 0) {
        t = a+b+c;
        s = d;
    } else {
        t = deep(n - 1, b, c, d, a + n);
        s = n;
    }
    return t + s;
}
int main(void) { report(deep(5, 1, 2, 3, 4)); return 0; }
