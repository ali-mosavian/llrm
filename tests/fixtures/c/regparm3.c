struct S4 { short a, b; };
long sized(char a, short b, long c, long d) { return a + b + c + d; }
int skip(float a, int b, double c, int d, int e) { return (int)a + b + (int)c + d + e; }
long long wide(long long a, int b) { return a + b; }
long long_result(long a, long b) { return a * b; }
struct S4 small(int a, int b) { struct S4 s; s.a = a; s.b = b; return s; }
extern char __far *screen(void);
void fill(int ch, int at) {
    char __far *p = screen();
    int o;
    for (o = 0; o <= 3998; o += 2) { p[o] = ch + (o & 15); p[o + 1] = at; }
}
char __far *far_result(char __far *p, int n) { return p + n; }
extern char __far *far_other(int n);
int caller(void) { char __far *p = far_other(3); return p[0] + p[1]; }
char __far *const_far(void) { return (char __far *)0xB8000000L; }
static int values[64];
long chop(int lo, int hi)
{
    long sum = 0;
    int i, mid;
    if (hi - lo < 2) return 0;
    mid = (lo + hi) / 2;
    for (i = lo; i < hi; ++i) sum += values[i] & mid;
    return sum + chop(lo, mid) + chop(mid, hi);
}
long stack_long(short a, short b, long c, unsigned char d, unsigned long e, long f) { return a + b * 3 + c * 5 + d * 7 + e * 11 + f * 13; }
extern void ext(int a);
int keep_far(char __far *p, int n) { int s = p[0]; ext(n); s += p[1]; ext(s); return s + p[2]; }
struct S4 take4(struct S4 s, int a) { return s; }
