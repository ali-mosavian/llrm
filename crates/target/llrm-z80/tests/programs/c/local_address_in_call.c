extern void report(long value);
__attribute__((noinline)) void g(int a, int b, int c, int d, int e, int f, int *p) { *p = a + b + c + d + e + f; }
int main(void)
{
    int x = 0;
    g(1, 2, 3, 4, 5, 6, &x);
    report(x);
    return 0;
}
