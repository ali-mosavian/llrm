// flags: -O2 -fsanitize=stack | -O2 -fsanitize=stack --target x86-code32
// A recursion that fits runs unchanged under -fsanitize=stack.
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
    report(deep(100));
    return 0;
}
