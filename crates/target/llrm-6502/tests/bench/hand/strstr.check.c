/* strstr: first occurrence of needle in haystack; empty needle gives the haystack; NULL if absent. */
extern char *strstr(const char *, const char *);
extern char *k_strstr(const char *, const char *);
extern char str[], mix[], pat[];
static char sa[900], sb[400];
static char *al(char *p) { return (char *)(((unsigned)p + 255u) & ~255u); }
static int t(const char *h, const char *n)
{
    return strstr(h, n) == k_strstr(h, n);
}
#define T(h, n) do { if (!t(h, n)) return i; i++; } while (0)
int main(void)
{
    int i = 1;
    unsigned j;
    char *p, *q;
    T("", "");
    T("", "a");
    T("abc", "");
    T("abc", "a");
    T("abc", "c");
    T("abc", "abc");
    T("abc", "abcd");     /* needle longer */
    T("abc", "bd");
    T("aab", "ab");       /* restart after a partial match */
    T("aaab", "aab");
    T("abababc", "ababc");
    T("abcabd", "abd");
    T(str, pat);          /* the bench pair */
    T(str, "world");
    T(str, "d");
    T(str, "l");
    T(str, "ll");
    T(str, "lll");
    T(str, "xyz");
    T(mix, "World");
    T(mix, "world");
    T(mix, "42!");
    T("\x80\xff\x80\xfe", "\xff\x80");
    T("\x80\xff\x80\xfe", "\x80\xfe");
    T("\x80\xff\x80\xfe", "\x7f");
    p = al(sa) + 250;     /* haystack crosses a page */
    for (j = 0; j < 20; ++j)
        p[j] = 'a' + j;
    p[20] = 0;
    T(p, "a");
    T(p, "fgh");
    T(p, "st");
    T(p, "t");
    T(p, "tu");
    T(p, "gi");
    q = al(sb) + 253;     /* needle crosses a page */
    q[0] = 'g'; q[1] = 'h'; q[2] = 'i'; q[3] = 'j'; q[4] = 0;
    T(p, q);
    q[3] = 'x';
    T(p, q);
    p = al(sa);
    for (j = 0; j < 600; ++j)
        p[j] = 'k';
    p[600] = 0;
    q = al(sb);
    for (j = 0; j < 300; ++j)
        q[j] = 'k';
    q[300] = 0;
    T(p, q);              /* match at 0 */
    q[299] = 'z';
    T(p, q);              /* near miss at every start */
    p[598] = 'z';
    T(p, q);              /* match at the very end... */
    q[299] = 'k'; q[298] = 'z'; q[300] = 'z'; q[301] = 0;
    T(p, q);
    p[600] = 0;
    T(p, "kz");
    T(p, "zk");
    T(p, "kkz");
    return 0;
}
