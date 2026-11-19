// flags: -O1 -march=i386 | -O2 -march=i386 | -Os -march=i386
// targets: dos
/* m16 O1/O2/Os: a function with an alloca and no frame cell returned with sp unrestored, so `ret` popped the alloca as the return address. */
extern void report(long value);

static void fill(int *p, int n)
{
    int i;
    for (i = 0; i < n; i++)
        p[i] = i * 3;
}

static int sum(int *p, int n)
{
    int i, s = 0;
    for (i = 0; i < n; i++)
        s += p[i];
    return s;
}

__attribute__((noinline)) int f(int n)
{
    int *p = __builtin_alloca(n * 4);
    fill(p, n);
    return sum(p, n);
}

int main(void)
{
    report(f(5));
    report(f(7));
    return 0;
}
