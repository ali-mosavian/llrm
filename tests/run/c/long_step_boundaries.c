// flags: -O1 | -O2 | -Os
// bar: none (new program)
// A long plus or minus one at every byte boundary (INC32/DEC32 carry up only when the byte below wraps), in a
// register, in a loop counter, and through a global.
extern void report(long value);

unsigned long up[8] = {0UL, 0xFFUL, 0xFFFFUL, 0xFFFFFFUL, 0xFFFFFFFFUL, 0x7FFFFFFFUL, 0x1FEUL, 0xFEFFUL};
unsigned long down[8] = {0UL, 1UL, 0x100UL, 0x10000UL, 0x1000000UL, 0x80000000UL, 0xFFFFUL, 0x10001UL};
unsigned long counter;

static unsigned long inc(unsigned long a) { return a + 1; }
static unsigned long dec(unsigned long a) { return a - 1; }

int main(void)
{
    volatile unsigned long *u = up;
    volatile unsigned long *d = down;
    unsigned long n;
    int i;

    for (i = 0; i < 8; ++i) {
        report((long)inc(u[i]));
        report((long)dec(d[i]));
    }
    counter = 0x00FFFFFEUL;
    for (i = 0; i < 4; ++i) {
        ++counter;
        report((long)counter);
    }
    counter = 0x01000002UL;
    for (i = 0; i < 4; ++i) {
        --counter;
        report((long)counter);
    }
    for (n = 0x10003UL; n != 0x0FFFEUL; --n)
        report((long)n);
    return 0;
}
