// flags: -fno-inline-functions
// A signed divide-and-remainder pair, then another divide: the first remainder is read after the second divide.
extern void report(long value);

static short f(short x, short y, short a, short b)
{
    short q = x / y, r = x % y, p = a / b, m = a % b;
    return (short)((p + r + q) ^ m);
}

static short in[][4] = {{-4829, 37, 1234, -7}, {4829, -37, -31000, 100}, {30000, 7, 999, 3}, {-1, 255, -25536, 9}};

int main(void)
{
    int i;
    for (i = 0; i < 4; ++i) {
        volatile short *v = in[i];
        report(f(v[0], v[1], v[2], v[3]));
    }
    return 0;
}
