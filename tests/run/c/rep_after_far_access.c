// flags: -O1 -march=i486 | -O2 -march=i486 | -Os -march=i486
// targets: dos
/* Near string moves and fills (`rep movs`, `rep stos`, `std; rep movs`) after far accesses: ES must be
   the near selector at the instruction, not the far one the last access left (backward moves wrote
   through it). Each case re-runs the far traffic first. */
extern void report(long value);
char s[140], d[140], buf[140];
short __far fa[100], fb[100];
volatile short vn;

static long sumb(char *p)
{
    long t = 0;
    short i;
    for (i = 0; i < 140; ++i) t += (long)(i + 1) * (unsigned char)p[i];
    return t;
}
static long far_traffic(void)
{
    long t = 0;
    short i;
    for (i = 0; i < 100; ++i) { fa[i] = i * 7 + 5; fb[i] = fa[i] + 1; }
    for (i = 0; i < 100; ++i) t += fb[i] - fa[i];
    return t;
}
static void init(void)
{
    short i;
    for (i = 0; i < 140; ++i) { s[i] = (i * 7 + 3) & 255; d[i] = 55; buf[i] = (i * 5 + 9) & 255; }
    far_traffic();
}
static void copy_up(short n) { short i; for (i = 0; i < n; ++i) d[i + 1] = s[i + 3]; }
static void fill(short n, char v) { short i; for (i = 0; i < n; ++i) d[i + 1] = v; }
static void move_down(short n) { short i; for (i = n - 1; i >= 0; --i) buf[i + 7] = buf[i]; }
static void move_up(short n) { short i; for (i = 0; i < n; ++i) buf[i] = buf[i + 7]; }

int main(void)
{
    static const short ns[8] = {0, 1, 2, 3, 4, 17, 18, 101};
    short t, n;
    for (t = 0; t < 8; ++t) {
        n = ns[t] + vn;
        init(); copy_up(n); report(sumb(d));
        init(); fill(n, (char)(n * 5 + 1)); report(sumb(d));
        init(); move_down(n); report(sumb(buf));
        init(); move_up(n); report(sumb(buf));
    }
    return 0;
}
