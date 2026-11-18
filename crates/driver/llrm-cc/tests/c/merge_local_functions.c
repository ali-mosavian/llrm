extern void report(long value);

__attribute__((noinline)) static int ge_a(int x, int y) { return (x >= 0) + y; }
__attribute__((noinline)) static int ge_b(int x, int y) { return ((x & 0x80000000) == 0) + y; }

int main(void)
{
    volatile int x = 5, y = 1;
    report(ge_a(x, y) + ge_b(x, y) * 2);
    return 0;
}
