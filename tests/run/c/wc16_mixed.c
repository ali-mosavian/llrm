// flags: -O2 -march=i486 -mabi=watcom
// link: wc16_callee.wc
/* One program of Open Watcom's own wcc objects and llrm's, on 16 bits: each call crosses the compilers in the register convention
   (AX, DX, BX, CX; a long or a far pointer in a pair; a struct result through SI; the callee pops) as the other compiler emits it. */
extern void report(long value);
struct S { int a, b, c; };
struct T { char x; };

extern int wc_six(int a, int b, int c, int d, int e, int f);
extern long wc_mixed(int a, long b, int c);
extern int wc_far(char __far *p, int a, int b);
extern struct S wc_result(int a, int b);
extern int wc_small(struct T t, int a);
extern int wc_by_value(struct S s, int a);
extern long wc_long(long a, long b);
extern int wc_calls_six(void);
extern long wc_calls_mixed(void);
extern int wc_calls_far(char __far *p);
extern int wc_calls_result(void);

int llrm_six(int a, int b, int c, int d, int e, int f) { return a + b * 2 + c * 3 + d * 4 + e * 5 + f * 6; }
long llrm_mixed(int a, long b, int c) { return b + b + a + c; }
int llrm_far(char __far *p, int a, int b) { return *p + a * 2 + b * 3; }
struct S llrm_result(int a, int b) { struct S s; s.a = a; s.b = b; s.c = a + b; return s; }

char byte = 7;

int main(void)
{
    struct S s = wc_result(7, 8);
    struct S t;
    struct T small;
    t.a = 1; t.b = 2; t.c = 3;
    small.x = 5;
    report(wc_six(1, 2, 3, 4, 5, 6));
    report(wc_mixed(1, 0x20003L, 4));
    report(wc_far((char __far *)&byte, 2, 3));
    report(s.a * 100 + s.b * 10 + s.c);
    report(wc_small(small, 6));
    report(wc_by_value(t, 9));
    report(wc_long(0x10001L, 0x20002L));
    report(wc_calls_six());
    report(wc_calls_mixed());
    report(wc_calls_far((char __far *)&byte));
    report(wc_calls_result());
    return 0;
}
