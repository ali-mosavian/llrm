// flags: -O2 -march=i486 -m32
// link: wc_callee.wc
/* One program of Open Watcom's own wcc386 objects and llrm's: each call crosses the compilers in the default convention,
   registers, an i64 pair, the stack and a struct through ESI, as the other compiler emits it. */
extern void report(long value);
struct S { int a, b, c; };
struct T { char x; };

extern int wc_mixed(int a, long long b, int c);
extern int wc_six(int a, int b, int c, int d, int e, int f);
extern struct S wc_result(int a, int b);
extern int wc_small(struct T t, int a);
extern int wc_by_value(struct S s, int a);
extern int wc_after_double(int a, double x, int b);
extern long long wc_wide(int a, long long b);
extern long long wc_calls_wide(void);
extern int wc_calls_mixed(void);
extern int wc_calls_six(void);
extern int wc_calls_result(void);
extern int wc_calls_double(void);

int llrm_mixed(int a, long long b, int c) { return a * 3 + (int)b * 7 + c * 11; }
long long llrm_wide(int a, long long b) { return b + b + b + a; }
int llrm_six(int a, int b, int c, int d, int e, int f) { return a + b * 2 + c * 3 + d * 4 + e * 5 + f * 6; }
struct S llrm_result(int a, int b) { struct S s; s.a = a; s.b = b; s.c = a + b; return s; }
int llrm_after_double(int a, double x, int b) { return a * 100 + (x > 2.5 ? 30 : 20) + b; }

int main(void)
{
    struct S s = wc_result(7, 8);
    struct S t;
    struct T small;
    long long wide;
    t.a = 1; t.b = 2; t.c = 3;
    small.x = 5;
    report(wc_mixed(1, 0x200000003LL, 4));
    report(wc_six(1, 2, 3, 4, 5, 6));
    report(s.a * 10000 + s.b * 100 + s.c);
    report(wc_small(small, 6));
    report(wc_by_value(t, 9));
    report(wc_after_double(2, 3.0, 4));
    wide = wc_wide(5, 0x100000002LL);
    report((long)(wide >> 32));
    report((long)wide);
    report(wc_calls_mixed());
    report(wc_calls_six());
    report(wc_calls_result());
    report(wc_calls_double());
    wide = wc_calls_wide();
    report((long)(wide >> 32));
    report((long)wide);
    return 0;
}
