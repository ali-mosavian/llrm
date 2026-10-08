// flags: -O0 | -O2 | -Os
// A bit field of a volatile object, read in a comparison, a sum and a condition (gcc.c-torture 20181120-1: `d != u.f1` was refused
// with "a bit field's address").
extern void report(long value);

struct S { unsigned a : 3; unsigned f : 15; unsigned long g : 14; };
union U { unsigned long whole; struct S s; };

volatile union U u;
volatile struct S v;
int d;

int main(void)
{
    u.whole = 0x4030201UL;
    v.a = 5;
    v.f = 12345;
    v.g = 321;
    d = (int)(u.s.f);
    report(d != u.s.f);
    report(d == u.s.f);
    report((long)v.a + v.f + v.g);
    v.f = v.f + 1;
    report(v.f);
    if (v.g > 300 && v.a == 5)
        report(1);
    else
        report(0);
    return 0;
}
