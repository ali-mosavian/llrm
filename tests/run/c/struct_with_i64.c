// flags: -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* A struct holding a long long, passed by value and returned (gcc.c-torture/pr30185, without its division, which is another bug). */
extern void report(long value);

typedef struct S { char a; long long b; } S;

S divided(S x, S y)
{
    S z;
    z.a = x.a + y.a;
    z.b = x.b * 2 + y.b;
    return z;
}

long long sum(S x, S y) { return x.b + y.b + x.a * 1000 + y.a * 100000; }

int main(void)
{
    S a, b, r;
    a.a = 1; a.b = 32LL;
    b.a = 2; b.b = 4LL;
    r = divided(a, b);
    report((long)r.b);
    report(r.a);
    report((long)sum(a, b));
    a.b = -8LL; b.b = -2LL;
    r = divided(a, b);
    report((long)r.b);
    return 0;
}
