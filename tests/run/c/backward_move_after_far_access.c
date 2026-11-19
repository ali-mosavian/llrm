// flags: -O2 -march=i486 | -Os -march=i486
// targets: dos
/* A backward near move (`std; rep movs`) after far accesses: ES held the far selector, so the
   move wrote through it and left buf[7] unchanged (sum 1253546, not 1253266). */
extern void report(long value);
char s[140], d[140], buf[140];
short near_a[100];
short __far far_a[100], far_b[100];

static long sumb(char *p)
{
    long t = 0;
    short i;
    for (i = 0; i < 140; ++i) t += (long)(i + 1) * (unsigned char)p[i];
    return t;
}
static long sumw(short *p)
{
    long t = 0;
    short i;
    for (i = 0; i < 100; ++i) t += (long)(i + 1) * p[i];
    return t;
}
static long sumf(short __far *p)
{
    long t = 0;
    short i;
    for (i = 0; i < 100; ++i) t += (long)(i + 1) * p[i];
    return t;
}
static void init(void)
{
    short i;
    for (i = 0; i < 140; ++i) { s[i] = (i * 7 + 3) & 255; d[i] = 200; buf[i] = (i * 5 + 9) & 255; }
    for (i = 0; i < 100; ++i) { near_a[i] = i * 3 + 1; far_a[i] = i * 7 + 5; far_b[i] = 0; }
}
static void odd_copy(short n) { short i; for (i = 0; i < n; ++i) d[i + 1] = s[i + 3]; }
static void odd_fill(short n, char v) { short i; for (i = 0; i < n; ++i) d[i + 1] = v; }
static void bytes_up(short n) { short i; for (i = 0; i < n; ++i) buf[i] = buf[i + 7]; }
static void bytes_down(short n) { short i; for (i = n - 1; i >= 0; --i) buf[i + 7] = buf[i]; }
static void far_to_near(short n) { short i; for (i = 0; i < n; ++i) near_a[i] = far_a[i]; }
static void near_to_far(short n) { short i; for (i = 0; i < n; ++i) far_a[i] = near_a[i]; }
static void far_to_far(short n) { short i; for (i = 0; i < n; ++i) far_b[i] = far_a[i]; }
static void far_up(short n, short by) { short i; for (i = 0; i < n; ++i) far_a[i] = far_a[i + by]; }

int main(void)
{
    static const short ns[2] = {0, 1};
    short t, n;
    for (t = 0; t < 2; ++t) {
        n = ns[t];
        init(); bytes_down(n); report(sumb(buf));
    }
    return 0;
}
