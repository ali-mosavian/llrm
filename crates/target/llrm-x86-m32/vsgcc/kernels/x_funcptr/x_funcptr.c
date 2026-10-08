extern void report(long value);

static int f0(int x) { return x + 3; } static int f1(int x) { return x * 5; } static int f2(int x) { return x ^ 0x55; } static int f3(int x) { return x >> 1; }
static int (*table[4])(int) = { f0, f1, f2, f3 };
long bench_x_funcptr(int n)
{
    int i, x = 1; long sum = 0;
    for (i = 0; i < n; ++i) { x = table[(i * 7 + (x & 3)) & 3](x) & 0xFFFF; sum += x; }
    return sum;
}

int main(void)
{
    report(bench_x_funcptr(1500));
    return 0;
}
