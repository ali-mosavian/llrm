// flags: -Os --cpu 486
/* Fill and copy loops as programs write them: clearing and scrolling a buffer, filling a
   palette, copying rows and records, and the loops that look alike but are not one move:
   a smear, a changed value, a second store, a stride that is not the cell, a source the body
   writes. Each prints a checksum that weighs every cell by its place. */
extern void report(long value);
short a[160], b[160], c[160], pal[64];
long L[40], M[40];

static void init(void)
{
    short i;
    for (i = 0; i < 160; ++i) { a[i] = i + 1000; b[i] = i * 3 + 1; c[i] = i * 5 + 2; }
}
static long sum(short *p, short n)
{
    long s = 0;
    short i;
    for (i = 0; i < n; ++i) s += (long)(i + 1) * p[i];
    return s;
}
static long sumlong(long *p, short n)
{
    long s = 0;
    short i;
    for (i = 0; i < n; ++i) s += (long)(i + 1) * p[i];
    return s;
}
static void clear_rows(short rows)
{
    short y, x;
    for (y = 0; y < rows; ++y)
        for (x = 0; x < 16; ++x) a[y * 16 + x] = 4660;
}
static void palette(short n, short w)
{
    short i;
    for (i = 0; i < n; ++i) pal[i] = w;
}
static void copy(short n) { short i; for (i = 0; i < n; ++i) a[i] = b[i]; }
static void scroll_up(short n) { short i; for (i = 0; i < n; ++i) a[i] = a[i + 16]; }
static void scroll_down(short n) { short i; for (i = n - 1; i >= 0; --i) a[i + 16] = a[i]; }
static void smear(short n) { short i; for (i = 0; i < n; ++i) a[i + 1] = a[i]; }
static void changed(short n) { short i; for (i = 0; i < n; ++i) a[i] = b[i] + 1; }
static void two_stores(short n) { short i; for (i = 0; i < n; ++i) { a[i] = b[i]; c[i] = 0; } }
static void strides(short n) { short i; for (i = 0; i < n / 2; ++i) a[2 * i] = b[i]; }
static void written(short n) { short i; for (i = 0; i < n; ++i) { b[i] = 0; a[i] = b[i]; } }
static void block(short w)
{
    short y, x;
    for (y = 0; y < 4; ++y)
        for (x = 0; x < w; ++x) a[y * 16 + x] = b[y * 20 + x];
}
static void copy_long(short n) { short i; for (i = 0; i < n; ++i) L[i] = M[i]; }
static void fill_long(short n, long v) { short i; for (i = 0; i < n; ++i) L[i] = v; }

int main(void)
{
    static const short ns[6] = {0, 1, 2, 17, 64, 100};
    short t, i, n;
    for (t = 0; t < 6; ++t) {
        n = ns[t];
        init(); clear_rows(n / 10); report(sum(a, 160));
        for (i = 0; i < 64; ++i) pal[i] = -1;
        palette(n < 64 ? n : 64, (short)(((n & 255) << 8) | 0x34)); report(sum(pal, 64));
        init(); copy(n); report(sum(a, 160));
        init(); scroll_up(n); report(sum(a, 160));
        init(); scroll_down(n); report(sum(a, 160));
        init(); smear(n); report(sum(a, 160));
        init(); changed(n); report(sum(a, 160));
        init(); two_stores(n); report(sum(a, 160) + sum(c, 160));
        init(); strides(n); report(sum(a, 160));
        init(); written(n); report(sum(a, 160) + sum(b, 160));
        init(); block(n / 10); report(sum(a, 160));
        for (i = 0; i < 40; ++i) { L[i] = 0x1000 + i; M[i] = 0x2000 + 3 * i; }
        copy_long(n < 40 ? n : 40); report(sumlong(L, 40));
        for (i = 0; i < 40; ++i) L[i] = -1;
        fill_long(n < 40 ? n : 40, 0x123456L + n); report(sumlong(L, 40));
    }
    return 0;
}
