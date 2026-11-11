// flags: -O1 -march=i486 | -O2 -march=i486 | -O2 -march=i486 -m32 | -Os -march=i486 -m32
// targets: dos
/* Copies and fills whose length is not a multiple of four, at every n & 3: the bytes after the last
   dword must land after it, not on the first ones (runtime copy tail restarted at the start: n=17 left
   the last byte stale). Runtime and constant sizes, forward and overlapping-backward moves, odd starts. */
extern void report(long value);

static char s[64], d[64];

static long sum(void)
{
    unsigned long h = 0;
    short i;
    for (i = 0; i < 64; ++i) h = h * 31 + (unsigned char)d[i];
    return (long)h;
}

static void init(void)
{
    short i;
    for (i = 0; i < 64; ++i) { s[i] = i + 1; d[i] = 100; }
}

static void copy_odd(short n) { short i; for (i = 0; i < n; ++i) d[i + 1] = s[i + 3]; }
static void fill_odd(short n, char v) { short i; for (i = 0; i < n; ++i) d[i + 1] = v; }
static void move_up(short n) { __builtin_memmove(d + 5, d + 1, n); }
static void move_down(short n) { __builtin_memmove(d + 1, d + 5, n); }
static void k5(void) { __builtin_memcpy(d + 1, s + 3, 5); }
static void k6(void) { __builtin_memcpy(d + 1, s + 3, 6); }
static void k7(void) { __builtin_memcpy(d + 1, s + 3, 7); }
static void k9(void) { __builtin_memcpy(d + 1, s + 3, 9); }
static void f5(void) { __builtin_memset(d + 1, 7, 5); }
static void f7(void) { __builtin_memset(d + 1, 7, 7); }
static void f9(void) { __builtin_memset(d + 1, 7, 9); }

int main(void)
{
    volatile short nn;
    short n;
    for (n = 0; n < 20; ++n) {
        nn = n;
        init(); copy_odd(nn); report(sum());
        init(); fill_odd(nn, (char)(n + 9)); report(sum());
        init(); move_up(nn); report(sum());
        init(); move_down(nn); report(sum());
    }
    init(); k5(); report(sum());
    init(); k6(); report(sum());
    init(); k7(); report(sum());
    init(); k9(); report(sum());
    init(); f5(); report(sum());
    init(); f7(); report(sum());
    init(); f9(); report(sum());
    return 0;
}
