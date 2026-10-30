extern char dst[64];
extern char *itoa(int value, char *buf, int radix);
extern char *k_itoa(int value, char *buf, int radix);

static char out[24];

/* Radix 10 writes '-' for negative values; other radixes print the value as unsigned, digits
   upper case. The hand routine returns buf. */
static const struct {
    int value;
    int radix;
} tab[] = {
    {0, 10}, {1, 10}, {9, 10}, {10, 10}, {-1, 10}, {-9, 10}, {-10, 10}, {255, 10}, {12345, 10}, {32767, 10},
    {-4829, 10}, {-32767, 10}, {(int)0x8000u, 10}, {0, 2}, {1, 2}, {5, 2}, {(int)0xAAAAu, 2}, {-1, 2},
    {(int)0x8000u, 2}, {0, 16}, {9, 16}, {10, 16}, {255, 16}, {0x1234, 16}, {0x7FFF, 16}, {-1, 16},
    {(int)0x8000u, 16}, {(int)0xBEEFu, 16}, {-4829, 16}, {100, 8}, {-100, 8}, {7, 3}, {-1, 7},
};

static void fill(char *p)
{
    unsigned char i;
    for (i = 0; i < sizeof out; ++i)
        p[i] = 0x55;
}

int main(void)
{
    unsigned i, j;
    char *r, *k;
    for (i = 0; i < sizeof tab / sizeof tab[0]; ++i) {
        fill(out);
        fill(dst);
        r = itoa(tab[i].value, dst, tab[i].radix);
        k = k_itoa(tab[i].value, out, tab[i].radix);
        if (r != dst || k != out)
            return i + 1;
        for (j = 0; j < sizeof out; ++j)
            if (dst[j] != out[j])
                return i + 1;
    }
    return 0;
}
