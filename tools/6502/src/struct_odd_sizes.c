// flags: -O2 -march=i486 | -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* A struct whose size is no whole number of words (3, 5, 7 bytes) passed by value: its last partial word was refused
   ("a struct of 3 bytes passed by value", gcc.c-torture/931004-11) and is built from pieces of 2 and 1 bytes. */
extern void report(long value);

struct tiny { char c, d, e; };
struct five { char a, b, c, d, e; };
struct seven { char a, b, c, d, e, f, g; };

int three(int n, struct tiny x, struct tiny y, long l)
{
    return n + x.c * 10 + x.d * 100 + x.e + y.c * 3 + y.d * 5 + y.e * 7 + (int)l;
}

int fives(struct five f, struct seven s)
{
    return f.a + f.e * 2 + s.a * 3 + s.g * 5 + s.d;
}

int main(void)
{
    struct tiny x, y;
    struct five f;
    struct seven s;
    x.c = 1; x.d = 2; x.e = 3;
    y.c = 4; y.d = 5; y.e = 6;
    f.a = 1; f.b = 2; f.c = 3; f.d = 4; f.e = 5;
    s.a = 1; s.b = 2; s.c = 3; s.d = 4; s.e = 5; s.f = 6; s.g = 7;
    report(three(1, x, y, 100L));
    report(fives(f, s));
    return 0;
}
