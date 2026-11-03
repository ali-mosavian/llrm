/* stricmp: hand vs k_stricmp vs a reference (fold to upper case, compare unsigned), by sign.
   Table cases plus a sweep of every byte value against case twins and neighbours. */
int __fastcall__ stricmp(const char *a, const char *b);
int k_stricmp(const char *a, const char *b);

static unsigned char ba[900], bb[900];

/* la, lb: string lengths; pos: position overwritten with va in a and vb in b (-1: none); oa, ob: offsets */
struct tc { unsigned la, lb; int pos; unsigned char va, vb, oa, ob; };

static int sgn(int x) { return x < 0 ? -1 : x > 0; }

static void build(unsigned char *b, unsigned off, unsigned len)
{
    unsigned i;

    for (i = 0; i < len; ++i)
        b[off + i] = (unsigned char)(1 + (i * 5) % 250);
    b[off + len] = 0;
}

static void setup(const struct tc *t)
{
    build(ba, t->oa, t->la);
    build(bb, t->ob, t->lb);
    if (t->pos >= 0) {
        ba[t->oa + t->pos] = t->va;
        bb[t->ob + t->pos] = t->vb;
    }
}

static const struct tc tcs[] = {
    {0, 0, -1, 0, 0, 0, 0}, {1, 1, 0, 'a', 'A', 0, 0}, {1, 1, 0, 'Z', 'z', 1, 0}, {1, 1, 0, '_', 'a', 0, 1},
    {1, 1, 0, 'a', '_', 0, 0}, {1, 1, 0, '@', '`', 0, 0}, {1, 1, 0, '[', '{', 1, 1}, {1, 1, 0, 'z', '{', 0, 0},
    {1, 1, 0, '`', 'A', 0, 0}, {1, 1, 0, 0xE1, 0xC1, 0, 0}, {1, 1, 0, 0xC1, 0xE1, 0, 0},
    {16, 16, 7, 'q', 'Q', 0, 1}, {16, 16, 15, 'm', 'N', 1, 0}, {255, 255, 254, 'x', 'X', 0, 0},
    {256, 256, 255, 'a', 'b', 0, 0}, {257, 257, 256, 'B', 'a', 255, 255}, {300, 300, -1, 0, 0, 255, 1},
    {300, 299, -1, 0, 0, 0, 0}, {299, 300, -1, 0, 0, 1, 1}, {600, 600, 599, 'y', 'Y', 3, 4}
};

static unsigned char up(unsigned char c) { return c >= 'a' && c <= 'z' ? c - 32 : c; }

static int ref(const unsigned char *p, const unsigned char *q)
{
    while (up(*p) == up(*q) && *p) { ++p; ++q; }
    return up(*p) < up(*q) ? -1 : up(*p) > up(*q);
}

static int one(const char *a, const char *b)
{
    int want = ref((const unsigned char *)a, (const unsigned char *)b);
    return sgn(stricmp(a, b)) == want && sgn(k_stricmp(a, b)) == want;
}

static const unsigned char twins[] = {0, 0x20, 'A', 'a', '_', '@', 0x80, 0xE1, 1};

int main(void)
{
    unsigned i, c, k;
    const struct tc *t;
    char x[3], y[3];

    for (i = 0; i < sizeof tcs / sizeof tcs[0]; ++i) {
        t = &tcs[i];
        setup(t);
        if (!one((char *)ba + t->oa, (char *)bb + t->ob))
            return i + 1;
    }
    x[1] = y[1] = 0;
    for (c = 1; c < 256; ++c) {
        x[0] = (char)c;
        for (k = 0; k < sizeof twins; ++k) {
            y[0] = (char)(twins[k] ? twins[k] : (c == 255 ? 1 : c + 1));
            if (k == 1)
                y[0] = (char)(c ^ 0x20);
            if (y[0] == 0)
                continue;
            if (!one(x, y) || !one(y, x))
                return 100 + c;
        }
    }
    return 0;
}
