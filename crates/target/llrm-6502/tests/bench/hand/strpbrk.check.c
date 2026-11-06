/* strpbrk: first byte of s that is in set (the terminator never matches); NULL otherwise. */
extern char *__fastcall__ strpbrk(const char *, const char *);
extern char *k_strpbrk(const char *, const char *);
extern char str[], mix[];
static char sa[800], sb[300];
static char *al(char *p) { return (char *)(((unsigned)p + 255u) & ~255u); }
static int t(const char *s, const char *set)
{
    return strpbrk(s, set) == k_strpbrk(s, set);
}
#define T(s, set) do { if (!t(s, set)) return i; i++; } while (0)
int main(void)
{
    int i = 1;
    unsigned j;
    char *p;
    T("", "");
    T("", "abc");
    T("abc", "");
    T("abc", "a");
    T("abc", "c");
    T("abc", "cb");
    T("abc", "xyz");
    T(str, "lo");
    T(str, "xd");
    T(str, "w");
    T(str, "zq");
    T(mix, "WH");
    T(mix, "!");
    T(mix, "hELO");
    T("\x80\xff\x01", "\xff");
    T("\x80\xff\x01", "\x01\xff");
    T("\x80\xff\x01", "\x7f");
    p = al(sa) + 250;     /* s crosses a page */
    for (j = 0; j < 20; ++j)
        p[j] = 'a' + j;
    p[20] = 0;
    T(p, "a");
    T(p, "kz");
    T(p, "t");
    T(p, "u");
    p = al(sa) + 255;
    p[0] = 'a'; p[1] = 'b'; p[2] = 0;
    T(p, "a"); T(p, "b"); T(p, "c");
    for (j = 0; j < 600; ++j)
        al(sa)[j] = 'k';
    al(sa)[600] = 0;
    T(al(sa), "x");
    al(sa)[599] = 'x';
    T(al(sa), "x");
    al(sa)[300] = 'y';
    T(al(sa), "yx");
    for (j = 0; j < 255; ++j)
        sb[j] = (char)(j + 1);
    sb[255] = 0;
    T(mix, sb);
    sb[0] = 'y';
    sb[1] = 0;
    T(al(sa), sb);
    return 0;
}
