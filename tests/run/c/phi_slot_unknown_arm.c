// flags: -O1 | -O2 -march=i486 | -O3 -march=i486 -m32 | -Os -march=i486 -m32
/* A slot's address joined with a pointer no fact follows, then read through the join, was taken to read no escaped object: DSE
   dropped the slot's `one = 1`, the descent started from garbage and the tree sum printed 2158 for 5736113 (bench bintree bq32). */
extern void report(long value);

static short store[903];
static short *volatile table = store;

static void add(short *at, short node)
{
    short *t = table;
    for (;;) {
        short side = t[3 * node] < t[3 * *at] ? 1 : 2;
        short *p = &t[3 * *at + side];
        if (*p == 0) {
            *p = node;
            return;
        }
        at = p;
    }
}

static long walk(short at, long depth)
{
    short *t = table;
    if (at == 0)
        return 0;
    return walk(t[3 * at + 1], depth + 1) + t[3 * at] * depth + walk(t[3 * at + 2], depth + 1);
}

int main(void)
{
    long x = 1;
    short i;
    short *t = table;

    for (i = 1; i <= 300; ++i) {
        x = (x * 25173 + 13849) & 0xFFFF;
        t[3 * i] = (short)(x & 0xFFF);
        if (i > 1) {
            short one = 1;
            add(&one, i);
        }
    }
    report(walk(1, 1));
    return 0;
}
