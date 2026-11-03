// flags: -O2 -march=i486
/* Two sibling calls to different callees under one test were tail-merged into one: g(1) called h. */
extern void report(long value);

static void __attribute__((noinline)) f(void) { report(1); }
static void __attribute__((noinline)) h(void) { report(2); }
static void __attribute__((noinline)) g(int n)
{
    if (n == 1)
        f();
    else if (n == 2)
        h();
}

int main(void)
{
    g(1);
    g(2);
    g(3);
    return 0;
}
