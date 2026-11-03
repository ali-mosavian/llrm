/* strcmp: hand vs k_strcmp vs a reference, by sign (the hand routine leaves a byte of s1 in A and +-1 in X). */
int __fastcall__ strcmp(const char *a, const char *b);
int k_strcmp(const char *a, const char *b);

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
    {0, 0, -1, 0, 0, 0, 0}, {1, 1, -1, 0, 0, 0, 0}, {0, 1, -1, 0, 0, 0, 0}, {1, 0, -1, 0, 0, 1, 0},
    {1, 1, 0, 5, 9, 0, 0}, {1, 1, 0, 9, 5, 1, 255}, {16, 16, 15, 0x7F, 0x80, 0, 1}, {16, 16, 0, 0x80, 0x7F, 1, 0},
    {255, 255, -1, 0, 0, 0, 1}, {255, 255, 254, 9, 3, 1, 1}, {256, 256, -1, 0, 0, 255, 0},
    {256, 256, 255, 3, 200, 0, 0}, {257, 257, 256, 200, 3, 255, 255}, {300, 300, 0, 255, 1, 0, 0},
    {300, 299, -1, 0, 0, 0, 0}, {299, 300, -1, 0, 0, 255, 255}, {600, 600, -1, 0, 0, 1, 0}, {600, 600, 599, 7, 8, 3, 4}
};

static int ref(const unsigned char *p, const unsigned char *q)
{
    while (*p == *q && *p) { ++p; ++q; }
    return *p < *q ? -1 : *p > *q;
}

int main(void)
{
    unsigned i;
    const struct tc *t;
    int want;

    for (i = 0; i < sizeof tcs / sizeof tcs[0]; ++i) {
        t = &tcs[i];
        setup(t);
        want = ref(ba + t->oa, bb + t->ob);
        if (sgn(strcmp((char *)ba + t->oa, (char *)bb + t->ob)) != want ||
            sgn(k_strcmp((char *)ba + t->oa, (char *)bb + t->ob)) != want)
            return i + 1;
    }
    return 0;
}
