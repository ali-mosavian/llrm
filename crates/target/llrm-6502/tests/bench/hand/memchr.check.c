/* memchr: hand vs k_memchr; marker position at either end, just past n, absent, n = 0, 255..513. */
void *__fastcall__ memchr(const void *p, int c, unsigned n);
void *k_memchr(const void *p, int c, unsigned n);

static unsigned char b[800];

/* offset, n, marker position (-1: none), search value (int; only the low byte counts) */
struct tc { unsigned off, n; int pos; int c; };
static const struct tc tcs[] = {
    {0, 0, -1, 0xFF}, {0, 0, 0, 0xFF}, {0, 1, 0, 0xFF}, {0, 1, 1, 0xFF}, {1, 10, 9, 0xFF}, {1, 10, 10, 0xFF},
    {0, 255, 254, 0xFF}, {0, 255, 255, 0xFF}, {1, 256, 255, 0xFF}, {1, 256, 256, 0xFF}, {255, 257, 256, 0xFF},
    {0, 300, 0, 0x1FF}, {3, 513, 512, 0xFF}, {3, 513, 513, -1}, {0, 600, -1, 0xFF}, {0, 300, 299, 0x00},
    {255, 300, 100, 0x80}, {1, 500, 256, 0x41}, {1, 500, -1, 0x41}
};

int main(void)
{
    unsigned i, j;
    const struct tc *t;
    void *r1, *r2, *ref;

    for (i = 0; i < sizeof tcs / sizeof tcs[0]; ++i) {
        t = &tcs[i];
        for (j = 0; j < 800; ++j)
            b[j] = 0x42;
        if (t->pos >= 0)
            b[t->off + t->pos] = (unsigned char)t->c;
        /* c == 0x41 is absent from the fill, so "not found" cases stay absent */
        ref = (t->pos >= 0 && (unsigned)t->pos < t->n) ? b + t->off + t->pos : 0;
        r1 = memchr(b + t->off, t->c, t->n);
        r2 = k_memchr(b + t->off, t->c, t->n);
        if (r1 != ref || r2 != ref)
            return i + 1;
    }
    return 0;
}
