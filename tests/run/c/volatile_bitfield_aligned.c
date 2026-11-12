// flags: -O0 | -O2 | -Os
// The twin of volatile_bitfield with fields on byte boundaries, which clang and SDCC lay out alike: the real cell against
// SDCC, as volatile_bitfield's struct differs between them (`f:15` at bit 3 under clang's packed layout, at a byte under SDCC's).
extern void report(long value);

struct S { unsigned a : 8; unsigned f : 16; unsigned long g : 16; };
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
