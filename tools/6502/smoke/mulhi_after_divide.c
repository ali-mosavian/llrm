// flags: -fno-inline-functions
// A 16-bit multiply (`__mulhi3`) with a product live across a divide pair, and one after it.
extern void report(long value);

static unsigned short f(unsigned short x, unsigned short y, unsigned short z, unsigned short w)
{
    unsigned short t = x * z;
    unsigned short q = x / y, r = x % y;
    return (unsigned short)(t + q * w + r * z);
}

static short g(short x, short y, short z, short w)
{
    short t = x * z;
    short q = x / y, r = x % y;
    return (short)(t + q * w - r * z);
}

static unsigned short in[][4] = {{61000, 37, 3, 5}, {30000, 7, 300, 11}, {65535, 255, 257, 2}, {12345, 100, 7, 65535}};

int main(void)
{
    int i;
    for (i = 0; i < 4; ++i) {
        volatile unsigned short *v = in[i];
        report(f(v[0], v[1], v[2], v[3]));
        report(g((short)v[0], (short)v[1] | 1, (short)v[2], (short)v[3]));
    }
    return 0;
}
