/* strncmp: hand vs k_strncmp vs a reference, by sign; n cuts before, at and after the first difference. */
int __fastcall__ strncmp(const char *a, const char *b, unsigned n);
int k_strncmp(const char *a, const char *b, unsigned n);

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

struct nc { struct tc t; unsigned n; };
static const struct nc ncs[] = {
    {{0, 0, -1, 0, 0, 0, 0}, 0}, {{0, 0, -1, 0, 0, 0, 0}, 5}, {{1, 1, 0, 5, 9, 0, 0}, 0},
    {{1, 1, 0, 5, 9, 0, 0}, 1}, {{1, 1, 0, 9, 5, 1, 255}, 1}, {{10, 10, 5, 0x7F, 0x80, 0, 1}, 5},
    {{10, 10, 5, 0x7F, 0x80, 0, 1}, 6}, {{10, 10, 5, 0x80, 0x7F, 1, 0}, 100}, {{255, 255, -1, 0, 0, 0, 1}, 255},
    {{255, 255, -1, 0, 0, 0, 1}, 256}, {{255, 255, 254, 9, 3, 1, 1}, 254}, {{255, 255, 254, 9, 3, 1, 1}, 255},
    {{256, 256, 255, 3, 200, 0, 0}, 255}, {{256, 256, 255, 3, 200, 0, 0}, 256}, {{257, 257, 256, 200, 3, 255, 255}, 256},
    {{257, 257, 256, 200, 3, 255, 255}, 257}, {{300, 300, -1, 0, 0, 255, 0}, 0xFFFF},
    {{300, 300, -1, 0, 0, 255, 0}, 300}, {{300, 300, -1, 0, 0, 255, 0}, 299}, {{300, 299, -1, 0, 0, 0, 0}, 299},
    {{300, 299, -1, 0, 0, 0, 0}, 300}, {{299, 300, -1, 0, 0, 1, 1}, 300}, {{600, 600, 511, 7, 8, 3, 4}, 511},
    {{600, 600, 511, 7, 8, 3, 4}, 512}, {{600, 600, 512, 8, 7, 3, 4}, 513}, {{600, 600, -1, 0, 0, 3, 4}, 0x8000}
};

static int ref(const unsigned char *p, const unsigned char *q, unsigned n)
{
    while (n--) {
        if (*p != *q) return *p < *q ? -1 : 1;
        if (!*p) return 0;
        ++p; ++q;
    }
    return 0;
}

int main(void)
{
    unsigned i;
    const struct tc *t;
    int want;

    for (i = 0; i < sizeof ncs / sizeof ncs[0]; ++i) {
        t = &ncs[i].t;
        setup(t);
        want = ref(ba + t->oa, bb + t->ob, ncs[i].n);
        if (sgn(strncmp((char *)ba + t->oa, (char *)bb + t->ob, ncs[i].n)) != want ||
            sgn(k_strncmp((char *)ba + t->oa, (char *)bb + t->ob, ncs[i].n)) != want)
            return i + 1;
    }
    return 0;
}
