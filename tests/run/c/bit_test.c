// flags: -O2 -march=i486 | -O2 -march=i486 -m32
/* A variable bit tested and flipped: the pin's BT and BTC forms (BT32rr, BTC16rr refused 'unmapped' in QCport). */
extern void report(long value);

static int __attribute__((noinline)) test_bit(unsigned short x, int n) { return (x & (1u << n)) != 0; }
static unsigned short __attribute__((noinline)) flip_bit(unsigned short x, int n) { return x ^ (unsigned short)(1u << n); }
static int __attribute__((noinline)) test_long(unsigned long x, int n) { return (int)((x >> n) & 1); }

int main(void)
{
    report(test_bit(0x0420, 5));
    report(test_bit(0x0420, 6));
    report(flip_bit(0x0420, 5));
    report(flip_bit(0x0420, 0));
    report(test_long(0x80000001ul, 31));
    report(test_long(0x80000001ul, 30));
    return 0;
}
