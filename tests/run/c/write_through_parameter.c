// flags: -O0 | -O2 | -Os
// A result that depends on a write through a parameter: a callee's effects are stated on it (`argmem: write`, #4202 /
// #4306), and a caller that read the stale value, or hoisted a load across the call, would print another number.
extern void report(long value);

static void set(int *p, int v) { *p = v; }
static void add_to(int *p, int v) { *p += v; }
static void swap(int *a, int *b)
{
    int t = *a;
    *a = *b;
    *b = t;
}
static int total(const int *p, int n)
{
    int s = 0, i;
    for (i = 0; i < n; i++)
        s += p[i];
    return s;
}
static void fill(int *p, int n)
{
    int i;
    for (i = 0; i < n; i++)
        p[i] = i * 3 + 1;
}

int main(void)
{
    int x = 1, y = 2, v[5];
    set(&x, 7);
    report(x);
    add_to(&x, 5);
    swap(&x, &y);
    report(x * 100L + y);
    fill(v, 5);
    report(total(v, 5));
    set(&v[2], 100);
    report(total(v, 5));
    return 0;
}
