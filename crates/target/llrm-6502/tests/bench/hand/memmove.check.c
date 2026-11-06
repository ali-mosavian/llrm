/* memmove: hand vs k_memmove vs a copy-through-temp reference; forward, backward and disjoint
   overlaps over page boundaries; compares result and every byte of all three buffers. */
void *__fastcall__ memmove(void *d, const void *s, unsigned n);
void *k_memmove(void *d, const void *s, unsigned n);

static unsigned char ba[1200], bb[1200], br[1200], tmp[800];

struct tc { unsigned d, s, n; };
static const struct tc tcs[] = {
    {0, 0, 10}, {1, 0, 10}, {0, 1, 10}, {5, 2, 0}, {2, 5, 0}, {2, 0, 1}, {0, 2, 1}, {1, 0, 255},
    {0, 1, 255}, {1, 0, 256}, {0, 1, 256}, {1, 0, 257}, {0, 1, 257}, {1, 0, 300}, {0, 1, 300},
    {255, 0, 300}, {0, 255, 300}, {256, 0, 300}, {0, 256, 300}, {2, 0, 512}, {0, 2, 512},
    {3, 0, 768}, {0, 3, 768}, {300, 0, 100}, {0, 300, 100}, {512, 0, 513}, {0, 512, 513},
    {255, 254, 513}, {254, 255, 513}
};

int main(void)
{
    unsigned i, j, k;
    const struct tc *t;
    void *r1, *r2;

    for (i = 0; i < sizeof tcs / sizeof tcs[0]; ++i) {
        t = &tcs[i];
        for (j = 0; j < 1200; ++j)
            ba[j] = bb[j] = br[j] = (unsigned char)(j * 5 + 0x81 + (j >> 8));
        r1 = memmove(ba + t->d, ba + t->s, t->n);
        r2 = k_memmove(bb + t->d, bb + t->s, t->n);
        for (k = 0; k < t->n; ++k)
            tmp[k] = br[t->s + k];
        for (k = 0; k < t->n; ++k)
            br[t->d + k] = tmp[k];
        if (r1 != ba + t->d || r2 != bb + t->d)
            return i + 1;
        for (j = 0; j < 1200; ++j)
            if (ba[j] != bb[j] || ba[j] != br[j])
                return i + 1;
    }
    return 0;
}
