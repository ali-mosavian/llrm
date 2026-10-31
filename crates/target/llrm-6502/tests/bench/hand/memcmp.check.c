/* memcmp: hand vs k_memcmp vs a reference, by sign (the hand routine leaves a byte of the operand in A
   and +-1 in X, so only the sign of the 16-bit result is defined). */
int __fastcall__ memcmp(const void *a, const void *b, unsigned n);
int k_memcmp(const void *a, const void *b, unsigned n);

static unsigned char ba[800], bb[800];

/* n, differing position (-1: none), value in a, value in b, offset a, offset b */
struct tc { unsigned n; int pos; unsigned char va, vb, oa, ob; };
static const struct tc tcs[] = {
    {0, -1, 0, 0, 0, 0}, {0, 0, 1, 2, 0, 0}, {1, -1, 0, 0, 0, 0}, {1, 0, 1, 2, 0, 0}, {1, 0, 2, 1, 1, 0},
    {16, 15, 0x7F, 0x80, 0, 1}, {16, 0, 0x80, 0x7F, 1, 0}, {255, -1, 0, 0, 0, 1}, {255, 254, 9, 3, 1, 1},
    {256, 255, 3, 200, 0, 0}, {256, 256, 3, 200, 0, 0}, {257, 256, 200, 3, 255, 0}, {300, 0, 0, 255, 0, 0},
    {300, 299, 255, 0, 1, 255}, {300, -1, 0, 0, 255, 1}, {512, 511, 1, 2, 0, 0}, {513, 512, 2, 1, 3, 4},
    {600, 255, 0x80, 0x81, 255, 255}, {600, -1, 0, 0, 1, 0}
};

static int sgn(int x) { return x < 0 ? -1 : x > 0; }

int main(void)
{
    unsigned i, j;
    const struct tc *t;
    int ref, r1, r2;

    for (i = 0; i < sizeof tcs / sizeof tcs[0]; ++i) {
        t = &tcs[i];
        for (j = 0; j < 800; ++j)
            ba[j] = bb[j] = (unsigned char)(j * 3 + 1 + (j >> 8));
        ref = 0;
        if (t->pos >= 0) {
            ba[t->oa + t->pos] = t->va;
            bb[t->ob + t->pos] = t->vb;
            if (t->pos < t->n)
                ref = t->va < t->vb ? -1 : 1;
        }
        /* the two windows must otherwise agree */
        for (j = 0; j < t->n; ++j)
            if ((int)j != t->pos)
                bb[t->ob + j] = ba[t->oa + j];
        r1 = memcmp(ba + t->oa, bb + t->ob, t->n);
        r2 = k_memcmp(ba + t->oa, bb + t->ob, t->n);
        if (sgn(r1) != ref || sgn(r2) != ref)
            return i + 1;
    }
    return 0;
}
