/* strnlen: hand vs k_strnlen vs min(len, max); max 0, 255, 256, 257, 65535; strings ending before, at, after max. */
unsigned __fastcall__ strnlen(const char *s, unsigned max);
unsigned k_strnlen(const char *s, unsigned max);

static unsigned char b[800];

struct tc { unsigned len, max, off; };
static const struct tc tcs[] = {
    {0, 0, 0}, {0, 5, 0}, {5, 0, 0}, {5, 5, 1}, {5, 4, 1}, {5, 6, 255}, {255, 255, 0}, {255, 256, 1},
    {256, 255, 0}, {256, 256, 255}, {256, 257, 1}, {300, 100, 0}, {300, 300, 255}, {300, 1000, 1},
    {300, 0xFFFF, 3}, {600, 0x00FF, 0}, {600, 0x0100, 1}, {600, 0x0101, 255}, {600, 511, 0}, {600, 512, 2},
    {600, 513, 255}, {600, 0xFFFE, 0}, {600, 0x8000, 7}
};

int main(void)
{
    unsigned i, j, want;
    const struct tc *t;

    for (i = 0; i < sizeof tcs / sizeof tcs[0]; ++i) {
        t = &tcs[i];
        for (j = 0; j < 800; ++j)
            b[j] = (unsigned char)(0x80 + (j % 120));
        b[t->off + t->len] = 0;
        want = t->len < t->max ? t->len : t->max;
        if (strnlen((char *)b + t->off, t->max) != want || k_strnlen((char *)b + t->off, t->max) != want)
            return i + 1;
    }
    return 0;
}
