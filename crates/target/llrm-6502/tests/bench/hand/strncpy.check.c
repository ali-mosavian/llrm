/* strncpy: result and every destination byte agree, including the zero padding and the bytes past n. */
extern char *__fastcall__ strncpy(char *, const char *, unsigned);
extern char *k_strncpy(char *, const char *, unsigned);
static char sa[800], da[800], db[800];
static char *al(char *p) { return (char *)(((unsigned)p + 255u) & ~255u); }
static int t(unsigned so, unsigned dof, unsigned len, unsigned n)
{
    char *s = al(sa) + so, *d1 = al(da) + dof, *d2 = al(db) + dof;
    char *r1, *r2;
    unsigned i;
    for (i = 0; i < len; ++i)
        s[i] = (char)(i % 251 + 1) | (i & 1 ? 0x80 : 0);
    s[len] = 0;
    for (i = 0; i < 700; ++i)       /* sentinel: shows overrun and missing padding */
        al(da)[i] = al(db)[i] = (char)0xA5;
    r1 = strncpy(d1, s, n);
    r2 = k_strncpy(d2, s, n);
    if (r1 != d1 || r2 != d2)
        return 0;
    for (i = 0; i < 700; ++i)
        if (al(da)[i] != al(db)[i])
            return 0;
    return 1;
}
#define T(so, dof, len, n) do { if (!t(so, dof, len, n)) return i; i++; } while (0)
int main(void)
{
    int i = 1;
    T(0, 0, 0, 0);
    T(0, 0, 5, 0);
    T(0, 0, 0, 1);
    T(0, 0, 0, 7);        /* empty src: all padding */
    T(0, 0, 11, 20);      /* 11 chars + 9 zeros */
    T(0, 0, 11, 11);      /* no room for the terminator */
    T(0, 0, 11, 12);
    T(0, 0, 11, 5);       /* truncated */
    T(0, 0, 11, 1);
    T(2, 3, 1, 2);
    T(250, 0, 10, 20);    /* src crosses a page */
    T(0, 250, 10, 20);    /* dst crosses a page, padding too */
    T(250, 252, 10, 12);
    T(0, 253, 3, 300);    /* padding crosses pages */
    T(0, 0, 10, 255);
    T(0, 0, 10, 256);
    T(0, 0, 10, 257);
    T(0, 0, 254, 255);
    T(0, 0, 255, 255);
    T(0, 0, 255, 256);
    T(0, 0, 256, 256);
    T(0, 0, 300, 256);
    T(0, 0, 300, 257);
    T(0, 0, 300, 299);
    T(0, 0, 300, 301);
    T(0, 0, 300, 400);
    T(1, 1, 255, 512);
    return 0;
}
