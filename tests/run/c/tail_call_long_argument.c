// flags: -O0 | -O2 | -Os
// A long computed in a function that ends in a tail call of report(): the call's argument registers were no longer read once
// the call became a jump, so the copy into hl was dropped as dead and the callee got a zero low word (expect 12640, 10360).
extern void report(long value);
void __attribute__((noinline)) kernel(unsigned count, int s0, int s1)
{
    report((long)count * 1000 + s0 + s1);
}
int main(void)
{
    kernel(12, 320, 320);
    kernel(11, -320, -320);
    return 0;
}
