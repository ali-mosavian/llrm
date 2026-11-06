/* memset: hand vs k_memset; odd and even counts, 255..513, fill with the int's low byte only. */
void *__fastcall__ memset(void *p, int c, unsigned n);
void *k_memset(void *p, int c, unsigned n);

static unsigned char da[1100], db[1100];

struct tc { unsigned off, n; int c; };
static const struct tc tcs[] = {
    {0, 0, 0x55}, {0, 1, 0x55}, {1, 2, 0}, {0, 3, 0xFF}, {255, 4, 0x1AB}, {1, 7, -1}, {0, 254, 0x7E},
    {0, 255, 0x80}, {3, 256, 0x01}, {255, 257, 0x00}, {1, 510, 0x42}, {0, 511, 0xFE}, {2, 512, 0x300},
    {255, 513, 0xC3}, {0, 1000, 0x5A}
};

int main(void)
{
    unsigned i, j;
    const struct tc *t;
    void *r1, *r2;

    for (i = 0; i < sizeof tcs / sizeof tcs[0]; ++i) {
        t = &tcs[i];
        for (j = 0; j < 1100; ++j)
            da[j] = db[j] = 0xAA;
        r1 = memset(da + t->off, t->c, t->n);
        r2 = k_memset(db + t->off, t->c, t->n);
        if (r1 != da + t->off || r2 != db + t->off)
            return i + 1;
        for (j = 0; j < 1100; ++j) {
            if (da[j] != db[j])
                return i + 1;
            if (j >= t->off && j < t->off + t->n ? da[j] != (unsigned char)t->c : da[j] != 0xAA)
                return i + 1;
        }
    }
    return 0;
}
