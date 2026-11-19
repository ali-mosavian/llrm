/* strcspn: length of the initial run of s free of set bytes; sets up to 255 bytes (cc65 ignores longer ones). */
extern unsigned strcspn(const char *, const char *);
extern unsigned k_strcspn(const char *, const char *);
extern char str[], mix[];
static char sa[800], sb[300];
static char *al(char *p) { return (char *)(((unsigned)p + 255u) & ~255u); }
static int t(const char *s, const char *set)
{
    return strcspn(s, set) == k_strcspn(s, set);
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
    T("abc", "abc");
    T("abc", "cba");
    T("abc", "xyz");
    T("abcd", "bca");
    T(str, "lo");         /* the bench pair */
    T(str, "helo");
    T(str, "helo wrd");
    T(mix, "Helo");
    T(mix, "hELO");       /* case matters */
    T("\x80\xff\x80", "\x80");
    T("\x80\xff\x80", "\xff\x80");
    T("\x80\xff\x80", "\x7f");
    T("aaaa", "a");
    p = al(sa) + 250;     /* s crosses a page */
    for (j = 0; j < 20; ++j)
        p[j] = 'a' + (j % 3);
    p[20] = 0;
    T(p, "abc");
    T(p, "ab");
    T(p, "a");
    p[11] = 'z';
    T(p, "abc");
    p = al(sa) + 255;
    p[0] = 'a'; p[1] = 'b'; p[2] = 0;
    T(p, "ab"); T(p, "a"); T(p, "b");
    for (j = 0; j < 600; ++j)      /* counts past 255 and past 256 */
        al(sa)[j] = 'k';
    al(sa)[600] = 0;
    T(al(sa), "k");
    T(al(sa), "xyk");
    T(al(sa), "x");
    al(sa)[256] = 'q';
    T(al(sa), "k");
    al(sa)[255] = 'q';
    T(al(sa), "k");
    T(al(sa), "kq");
    for (j = 0; j < 200; ++j)      /* long set, match at its end */
        sb[j] = 'A' + (j % 40) + 1;
    sb[200] = 'k';
    sb[201] = 0;
    T(al(sa), sb);
    sb[200] = 0;
    T(al(sa), sb);
    for (j = 0; j < 255; ++j)
        sb[j] = (char)(j + 1);
    sb[255] = 0;
    T(al(sa), sb);
    T(mix, sb);
    return 0;
}
