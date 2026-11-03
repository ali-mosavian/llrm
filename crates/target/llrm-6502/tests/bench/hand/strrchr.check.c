/* strrchr: last-occurrence pointer, high-bit bytes, page crossings.
   KNOWN cc65 BUG: the hand routine returns NULL for c == 0, though funcref says the terminator
   is part of the string. k_strrchr follows the docs; for c == 0 the check pins the split
   (hand NULL, kernel = terminator) instead of equality. */
extern char *strrchr(const char *, int);
extern char *k_strrchr(const char *, int);
extern char str[], mix[];
static char sa[800];
static char *al(char *p) { return (char *)(((unsigned)p + 255u) & ~255u); }
static int t(const char *s, int c)
{
    const char *e = s;
    if ((char)c == 0) {
        while (*e) ++e;
        return strrchr(s, c) == 0 && k_strrchr(s, c) == e;
    }
    return strrchr(s, c) == k_strrchr(s, c);
}
#define T(s, c) do { if (!t(s, c)) return i; i++; } while (0)
int main(void)
{
    int i = 1;
    unsigned j;
    char *p;
    T("", 0);
    T("", 'a');
    T("a", 'a');
    T("a", 0);
    T("a", 'b');
    T(str, 'h');          /* first */
    T(str, 'd');          /* last char */
    T(str, 'o');          /* last of two */
    T(str, 0);            /* terminator */
    T(str, 'z');
    T(mix, 'W');
    T(mix, 'w');
    T(mix, '!');
    T("\x80\xff\x01", 0xff);
    T("\x80\xff\x01", 0x80);
    T("\x80\xff\x01", 0x7f);
    T("abc", 0x161);      /* only the low byte of c counts: 'a' + 0x100 */
    T("abc", 0x100);      /* low byte 0 = terminator */
    T("abc", -1);
    p = al(sa) + 250;     /* string crosses a page */
    for (j = 0; j < 20; ++j)
        p[j] = 'a' + j;
    p[20] = 0;
    T(p, 'a');
    T(p, 'f');
    T(p, 'j');
    T(p, 'k');
    T(p, 't');
    T(p, 0);
    T(p, 'z');
    p = al(sa) + 255;
    p[0] = 'x'; p[1] = 'y'; p[2] = 0;
    T(p, 'x'); T(p, 'y'); T(p, 0); T(p, 'q');
    p = al(sa);
    p[255] = 0;           /* terminator on the last byte of a page */
    for (j = 0; j < 255; ++j)
        p[j] = 'm';
    T(p, 0);
    T(p, 'm');
    T(p, 'n');
    for (j = 0; j < 600; ++j)
        p[j] = 'm';
    p[599] = 'b';
    p[600] = 0;
    T(p, 'b');
    T(p, 0);
    return 0;
}
