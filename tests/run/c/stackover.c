// flags: -O2 -fsanitize=stack
// Unbounded recursion under -fsanitize=stack ends in the runtime's "Stack Overflow!" with status
// 1, where it would run off the stack into the program's data. The pad keeps it a recursion.
extern void report(long value);
int deep(int n)
{
    volatile char pad[48];
    pad[0] = (char)n;
    if (n == 0) return 0;
    return 1 + deep(n - 1) + pad[0] - (char)n;
}
int main(void)
{
    report(1);
    report(deep(30000));
    report(2);
    return 0;
}
