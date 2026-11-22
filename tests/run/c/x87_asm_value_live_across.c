// flags: -O2
// targets: x86-m32 x86-m64
/* An x87 value that is an asm block's st(0) input and also read after it, across a branch: the allocator put the value's spill at the call,
   behind the input that consumes it, "floating stack input is unavailable" (QB pfpu.c `fx_exp`). Prints 6, 7, 0. */
extern void report(long value);

typedef long double X;

static X rnd(X x)
{
    __asm__ volatile("frndint" : "+t"(x));
    return x;
}

static X __attribute__((noinline)) f(X x, X limit)
{
    X t = x * 1.5L;
    if (t >= limit)
        return 0;
    return rnd(t) + t;
}

int main(void)
{
    report((long)f(2.0L, 100));
    report((long)f(2.4L, 100));
    report((long)f(100.0L, 100));
    return 0;
}
