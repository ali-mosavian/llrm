/* memcpy: hand routine vs k_memcpy on 15 copies; compares result and every byte of both buffers. */
void *__fastcall__ memcpy(void *d, const void *s, unsigned n);
void *k_memcpy(void *d, const void *s, unsigned n);

static unsigned char sb[1100], da[1100], db[1100];

struct tc { unsigned so, dof, n; };
static const struct tc tcs[] = {
    {0, 0, 0}, {0, 0, 1}, {1, 2, 2}, {3, 1, 3}, {0, 0, 127}, {5, 7, 128}, {0, 1, 255}, {1, 0, 256},
    {255, 255, 257}, {3, 250, 300}, {0, 0, 511}, {1, 1, 512}, {7, 255, 513}, {256, 0, 600}, {0, 300, 768}
};

int main(void)
{
    unsigned i, j;
    const struct tc *t;
    void *r1, *r2;

    for (i = 0; i < 1100; ++i)
        sb[i] = (unsigned char)(i * 7 + 0x80 + (i >> 8));
    for (i = 0; i < sizeof tcs / sizeof tcs[0]; ++i) {
        t = &tcs[i];
        for (j = 0; j < 1100; ++j)
            da[j] = db[j] = 0xAA;
        r1 = memcpy(da + t->dof, sb + t->so, t->n);
        r2 = k_memcpy(db + t->dof, sb + t->so, t->n);
        if (r1 != da + t->dof || r2 != db + t->dof)
            return i + 1;
        for (j = 0; j < 1100; ++j) {
            if (da[j] != db[j])
                return i + 1;
            if (j >= t->dof && j < t->dof + t->n ? da[j] != sb[t->so + j - t->dof] : da[j] != 0xAA)
                return i + 1;
        }
    }
    return 0;
}
