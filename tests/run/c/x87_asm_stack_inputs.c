// flags: -O2 -march=i486
// targets: x86-m32 x86-m64
/* An inline asm block that takes st(1) as an input ("u"): `fscale` leaves it, `fyl2x` and `fpatan` consume it. The block was refused
   ("inline assembly that takes st(1) as an input"); a stack order the wrong way gives 32 for the first line, 8 for the third. */
extern void report(long value);

typedef long double X;

static X __attribute__((noinline)) scale(X x, X e)
{
    __asm__ volatile("fscale" : "+t"(x) : "u"(e));
    return x;
}

static X __attribute__((noinline)) times_log2(X x, X y)
{
    X r;
    __asm__ volatile("fyl2x" : "=t"(r) : "0"(x), "u"(y) : "st(1)");
    return r;
}

static X __attribute__((noinline)) arctan(X x)
{
    X r;
    __asm__ volatile("fpatan" : "=t"(r) : "0"((X)1), "u"(x) : "st(1)");
    return r;
}

int main(void)
{
    report((long)scale(3, 4));
    report((long)scale(40, -3));
    report((long)times_log2(8, 2));
    report((long)(arctan(1) * 4000));
    X same = 3;
    report((long)scale(same, same));
    /* st(1) left on the stack by each block would overflow the eight entries. */
    long total = 0;
    for (int i = 0; i < 20; i++) total += (long)scale(1, i & 3);
    report(total);
    return 0;
}
