// flags: -O2 -march=i486 -m32 -mabi=sysv
/* -mabi=sysv: the stack convention with a struct result through the first argument, which the callee pops (gcc -m32 on Linux). */
extern void report(long value);
struct S1 { char x; };
struct S8 { int x, y; };
struct S12 { int a, b, c; };

struct S1 one(int a) { struct S1 s; s.x = a; return s; }
struct S8 two(int a, int b) { struct S8 s; s.x = a; s.y = b; return s; }
struct S12 three(int a, int b) { struct S12 s; s.a = a; s.b = b; s.c = a + b; return s; }
long long wide(int a, long long b, int c) { return b + b + b + a + c; }
int by_value(struct S12 s, struct S1 t, int a) { return s.a + s.c + t.x + a; }
int six(int a, int b, int c, int d, int e, int f) { return a + b * 2 + c * 3 + d * 4 + e * 5 + f * 6; }

int main(void)
{
    struct S1 a = one(7);
    struct S8 b = two(7, 8);
    struct S12 c = three(7, 8);
    struct S12 w;
    struct S1 t;
    long long l = wide(1, 0x200000003LL, 4);
    w.a = 1; w.b = 2; w.c = 3;
    t.x = 5;
    report(a.x);
    report(b.x * 100 + b.y);
    report(c.a * 10000 + c.b * 100 + c.c);
    report((long)(l >> 32));
    report((long)l);
    report(by_value(w, t, 9));
    report(six(1, 2, 3, 4, 5, 6));
    return 0;
}
