// flags: -O2 -march=i486 -mabi=ia16
// targets: x86-m16
/* -mabi=ia16: gcc-ia16's convention, the stack one with a struct result of more than 4 bytes through a near first argument. */
extern void report(long value);
struct S { int a, b, c; };
struct W { int a, b; };

struct S three(int a, int b) { struct S s; s.a = a; s.b = b; s.c = a + b; return s; }
struct W two(int a, int b) { struct W s; s.a = a; s.b = b; return s; }
long wide(int a, long b, int c) { return b + b + a + c; }
int by_value(struct S s, int a) { return s.a + s.b * 2 + s.c * 3 + a * 4; }
int six(int a, int b, int c, int d, int e, int f) { return a + b * 2 + c * 3 + d * 4 + e * 5 + f * 6; }

int main(void)
{
    struct S c = three(7, 8);
    struct W w = two(3, 4);
    struct S t;
    t.a = 1; t.b = 2; t.c = 3;
    report(c.a * 100 + c.b * 10 + c.c);
    report(w.a * 10 + w.b);
    report(wide(1, 0x20003L, 4));
    report(by_value(t, 9));
    report(six(1, 2, 3, 4, 5, 6));
    return 0;
}
