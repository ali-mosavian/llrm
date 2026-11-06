// flags: -O2 -march=i486 | -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv | -Os -march=i486 -m32
/* A function that leaves before it touches its frame sets the frame up after that test: the early path
   builds none, and the late one builds and takes back its own, whatever the call depth. */
extern void report(long value);
static long depth(long *cell)
{
    return *cell;
}
static long f(long n, long *out)
{
    long a[16];
    long i, sum = 0;
    if (n == 0) return 7;
    for (i = 0; i < 16; ++i) a[i] = n * i;
    a[3] = depth(out) + n;
    for (i = 0; i < 16; ++i) sum += a[i];
    return sum;
}
static long g(long n, long *out)
{
    if (n > 3) return n;
    return f(n, out) + g(n + 1, out);
}
int main(void)
{
    long x = 5;
    report(f(0, &x));
    report(f(2, &x));
    report(g(0, &x));
    return 0;
}
