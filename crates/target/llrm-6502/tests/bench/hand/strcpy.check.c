/* strcpy: result and every byte of the destination agree. */
extern char *strcpy(char *, const char *);
extern char *k_strcpy(char *, const char *);
static char sa[800], da[800], db[800];
static char *al(char *p) { return (char *)(((unsigned)p + 255u) & ~255u); }
/* src of len chars at page offset so, dst at page offset do */
static int t(unsigned so, unsigned dof, unsigned len)
{
    char *s = al(sa) + so, *d1 = al(da) + dof, *d2 = al(db) + dof;
    char *r1, *r2;
    unsigned i;
    for (i = 0; i < len; ++i)
        s[i] = (char)(i % 251 + 1) | (i & 1 ? 0x80 : 0);
    s[len] = 0;
    for (i = 0; i < 512; ++i)
        al(da)[i] = al(db)[i] = (char)0xA5;
    r1 = strcpy(d1, s);
    r2 = k_strcpy(d2, s);
    if (r1 != d1 || r2 != d2)
        return 0;
    for (i = 0; i < 512; ++i)
        if (al(da)[i] != al(db)[i])
            return 0;
    for (i = 0; i <= len; ++i)
        if (d1[i] != s[i])
            return 0;
    return 1;
}
#define T(so, dof, len) do { if (!t(so, dof, len)) return i; i++; } while (0)
int main(void)
{
    int i = 1;
    T(0, 0, 0);
    T(0, 0, 1);
    T(3, 5, 11);
    T(0, 0, 100);
    T(250, 0, 10);        /* src crosses a page */
    T(0, 250, 10);        /* dst crosses a page */
    T(250, 252, 10);      /* both */
    T(255, 255, 1);
    T(255, 255, 0);
    T(1, 0, 254);
    T(0, 0, 255);
    T(0, 0, 256);
    T(0, 0, 257);
    T(7, 9, 300);
    T(250, 3, 255);
    return 0;
}
