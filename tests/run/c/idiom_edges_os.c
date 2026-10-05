// flags: -Os --cpu 486
/* The same moves at their edges: odd byte counts, starts that are not aligned, no trips and
   one, the far segment on either side of a copy and both sides of one. */
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
    static const short ns[5] = {0, 1, 3, 17, 101};
    static const short ms[5] = {0, 1, 2, 17, 100};
    short t, n;
    for (t = 0; t < 5; ++t) {
        n = ns[t];
        init(); odd_copy(n); report(sumb(d));
        init(); odd_fill(n, (char)(n * 5 + 1)); report(sumb(d));
        init(); bytes_up(n); report(sumb(buf));
        init(); bytes_down(n); report(sumb(buf));
    }
    for (t = 0; t < 5; ++t) {
        n = ms[t];
        init(); far_to_near(n); report(sumw(near_a));
        init(); near_to_far(n); report(sumf(far_a));
        init(); far_to_far(n); report(sumf(far_b));
        init(); far_up(n, (100 - n) / 2); report(sumf(far_a));
    }
    return 0;
}
