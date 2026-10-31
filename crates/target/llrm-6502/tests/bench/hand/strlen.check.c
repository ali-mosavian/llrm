/* strlen: hand vs k_strlen vs the construction length; lengths 0..513 at page-crossing offsets, high-bit bytes. */
unsigned __fastcall__ strlen(const char *s);
unsigned k_strlen(const char *s);

static unsigned char b[800];

struct tc { unsigned len, off; };
static const struct tc tcs[] = {
    {0, 0}, {1, 0}, {2, 1}, {3, 255}, {127, 0}, {128, 1}, {254, 0}, {255, 0}, {255, 1}, {256, 0},
    {256, 255}, {257, 1}, {300, 255}, {511, 3}, {512, 0}, {513, 255}
};

int main(void)
{
    unsigned i, j;
    const struct tc *t;

    for (i = 0; i < sizeof tcs / sizeof tcs[0]; ++i) {
        t = &tcs[i];
        for (j = 0; j < 800; ++j)
            b[j] = (unsigned char)(0x80 + (j % 120));
        b[t->off + t->len] = 0;
        if (strlen((char *)b + t->off) != t->len || k_strlen((char *)b + t->off) != t->len)
            return i + 1;
    }
    return 0;
}
