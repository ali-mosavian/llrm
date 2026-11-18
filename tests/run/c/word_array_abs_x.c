// flags: -O2 -march=i486 | -O2 -march=i486 -m32 | -Os -march=i486 -m32
/* A word and a long of a global array read and written at an index a byte holds: g+k,x for each byte. */
extern void report(long value);
short w[128];
long l[64];
unsigned char seed = 7;

static short get_w(short i) { return w[(i * 5 + 3) & 127]; }
static long get_l(short i) { return l[(i * 5 + 3) & 63]; }
static void put_w(short i, short v) { w[(i * 3) & 127] = v; }
static void put_l(short i, long v) { l[(i * 3) & 63] = v; }

int main(void)
{
    short i;
    long s = 0, t = 0;

    for (i = 0; i < 200; ++i) {
        put_w(i, (short)(i * 257 + seed));
        put_l(i, (long)i * 65539L - seed);
    }
    for (i = 0; i < 300; ++i) {
        s += get_w(i);
        t += get_l(i);
    }
    report(s);
    report(t);
    report(w[127]);
    report(l[63]);
    return 0;
}
