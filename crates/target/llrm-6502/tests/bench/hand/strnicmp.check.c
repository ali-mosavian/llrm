/* strnicmp: sign of result vs k_strnicmp (the hand routine returns only the sign in X; A is junk). */
extern int __fastcall__ strnicmp(const char *, const char *, unsigned);
extern int k_strnicmp(const char *, const char *, unsigned);
extern char mix[], mix2[];
static char pa[700], pb[700];
static int sgn(int v) { return v < 0 ? -1 : v > 0; }
static char *al(char *p) { return (char *)(((unsigned)p + 255u) & ~255u); }
static int t(const char *a, const char *b, unsigned n)
{
    int h = strnicmp(a, b, n), k = k_strnicmp(a, b, n);
    return sgn(h) == sgn(k) && (h == 0) == (k == 0);
}
#define T(a, b, n) do { if (!t(a, b, n)) return i; i++; } while (0)
int main(void)
{
    int i = 1;
    unsigned j;
    char *x, *y;
    T("", "", 0);
    T("", "", 5);
    T("abc", "ABC", 3);
    T("abc", "ABD", 3);
    T("abd", "ABC", 3);
    T("abc", "abd", 2);           /* differs only past count */
    T("abc", "abd", 0);
    T("abc", "ab", 5);
    T("ab", "abc", 5);
    T("a", "", 1);
    T("", "a", 1);
    T(mix, mix2, 15);
    T(mix, mix2, 16);
    T("\xc1", "\xe1", 1);         /* high bit: not folded */
    T("\xff", "\x01", 1);
    T("_", "a", 1);               /* between 'Z' and 'a': upper-folding order */
    T("a", "_", 1);
    T("[", "A", 1);
    T("@", "`", 1);
    T("`", "@", 1);
    T("z", "Z", 9);
    T("Z", "z{", 2);
    T("{", "Z", 1);
    x = al(pa) + 250; y = al(pb) + 252;   /* both cross a page */
    for (j = 0; j < 12; ++j) { x[j] = 'a' + j; y[j] = 'A' + j; }
    x[12] = y[12] = 0;
    T(x, y, 12);
    T(x, y, 40);
    y[10] = 'x';
    T(x, y, 10);
    T(x, y, 11);
    y[10] = 'A' + 10;
    for (j = 0; j < 299; ++j) { pa[j] = 'q'; pb[j] = 'Q'; }
    pa[299] = pb[299] = 0;
    T(pa, pb, 255);
    T(pa, pb, 256);
    T(pa, pb, 257);
    T(pa, pb, 299);
    T(pa, pb, 300);
    T(pa, pb, 65535u);
    pb[280] = 'r';
    T(pa, pb, 280);
    T(pa, pb, 281);
    T(pa, pb, 300);
    pb[255] = 'a';
    T(pa, pb, 255);
    T(pa, pb, 256);
    pb[255] = 'Q'; pb[256] = 'a';
    T(pa, pb, 256);
    T(pa, pb, 257);
    return 0;
}
