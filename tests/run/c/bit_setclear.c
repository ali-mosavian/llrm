// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* A variable bit set and cleared: the pin's BTS and BTR forms (BTS16rr, BTR16rr refused 'unmapped' in QCport's r_walk). */
extern void report(long value);

static unsigned short __attribute__((noinline)) set_bit(unsigned short x, int n) { return x | (unsigned short)(1u << n); }
static unsigned short __attribute__((noinline)) clear_bit(unsigned short x, int n) { return x & (unsigned short)~(1u << n); }
static unsigned long __attribute__((noinline)) set_long(unsigned long x, int n) { return x | (1ul << n); }
static unsigned long __attribute__((noinline)) clear_long(unsigned long x, int n) { return x & ~(1ul << n); }

int main(void)
{
    report(set_bit(0x0420, 6));
    report(set_bit(0x0420, 5));
    report(clear_bit(0x0420, 5));
    report(clear_bit(0x0420, 0));
    report(set_long(0, 31));
    report(clear_long(0xfffffffful, 0));
    return 0;
}
