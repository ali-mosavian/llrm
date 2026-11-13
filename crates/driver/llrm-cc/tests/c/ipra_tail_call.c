extern void report(long);
__attribute__((noinline)) static int g(int x, int y) { return x < y ? 5 : 6; }
__attribute__((noinline)) static int f1(int x, int y) { return g(x, y); }
__attribute__((noinline)) static int f2(int x, int y) { return g(x, y); }
int chain_tail(int a, int b)
{
    int h = f1(a, b);
    return h * 31 + f2(a, b);
}
int main(void) { volatile int a = 3, b = 4; report(chain_tail(a, b)); return 0; }
