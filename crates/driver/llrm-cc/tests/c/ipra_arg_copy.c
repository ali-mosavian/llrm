extern void report(long);
__attribute__((noinline)) static signed char f(int x, int y) { return (x < y) ? 5 : 6; }
__attribute__((noinline)) static signed char g(int x, int y) { return (x == y) ? 5 : 6; }
int chain(int a, int b)
{
    unsigned h = 0;
    h = h * 31u + (unsigned)f(a, b);
    h = h * 31u + (unsigned)g(a, b);
    h = h * 31u + (unsigned)f(a, b);
    h = h * 31u + (unsigned)g(a, b);
    return (int)h;
}
int main(void) { volatile int a = 3, b = 4; report(chain(a, b)); return 0; }
