/* A word or dword stored in n cells by a loop: the cells filled and no others. */
extern void report(long value);
short a[64];
long b[64];
void fillw(short n, short v) { short i; for (i = 0; i < n; ++i) a[i] = v; }
void filll(short n, long v) { short i; for (i = 0; i < n; ++i) b[i] = v; }
int main(void)
{
    static const short counts[6] = {0, 1, 2, 17, 63, 64};
    short t, i, c;
    long s;
    for (t = 0; t < 6; ++t) {
        for (i = 0; i < 64; ++i) { a[i] = 0; b[i] = 0; }
        fillw(counts[t], 4660);
        filll(counts[t], 305419896L);
        s = 0; c = 0;
        for (i = 0; i < 64; ++i) { s += a[i]; if (b[i]) ++c; }
        report(counts[t]); report(s); report(c); report(b[63]);
    }
    return 0;
}
