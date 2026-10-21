// flags: -O2 | -O3 | -Os
/* An inlined body's `tail` call (its own frame is not the caller's) read a caller's local array once inlined at a plain call site: the loads after it were folded to the stored 1, not the 5 the callee wrote. */
extern void report(long value);

static void __attribute__((noinline)) set(int *p)
{
    p[0] = 5;
}

static void via(int *p)
{
    set(p);
}

int __attribute__((noinline)) run(void)
{
    int q[2];

    q[0] = 1;
    q[1] = 2;
    via(q);
    return q[0];
}

int main(void)
{
    report(run());
    return 0;
}
