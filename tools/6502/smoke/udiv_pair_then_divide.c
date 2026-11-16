// flags: -fno-inline-functions
// An unsigned divide-and-remainder pair, then another divide: the first remainder is read after the second divide
// (61000 / 37, then 50000 / 7: 120 where 74 is right).
extern void report(long value);

static unsigned short f(unsigned short x, unsigned short y, unsigned short a, unsigned short b)
{
    unsigned short q = x / y, r = x % y, p = a / b, m = a % b;
    return (unsigned short)((p + r + q) ^ m);
}

static unsigned short in[][4] = {{61000, 37, 50000, 7}, {30000, 7, 999, 3}, {65535, 255, 40000, 9}, {1, 3, 65535, 2}};

int main(void)
{
    int i;
    for (i = 0; i < 4; ++i) {
        volatile unsigned short *v = in[i];
        report(f(v[0], v[1], v[2], v[3]));
    }
    return 0;
}
