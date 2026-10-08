// flags: -O0 | -O2
// A struct wider than 16 bytes is passed by value as one byval pointer, the caller copying it onto the stack where its words would
// lie (clang's X86_32ABIInfo: expanded only up to 128 bits). The callee's copy is its own: it writes it, the caller's is unchanged.
extern void report(long value);

struct big { long a[10]; };
struct odd { char c[41]; };

static long sum(struct big b, long x)
{
    long s = x;
    int i;

    for (i = 0; i < 10; i++)
        s += b.a[i];
    b.a[0] = 99;
    return s + b.a[0];
}

static long again(long x, struct big b, struct odd o, long y)
{
    return sum(b, x) + o.c[40] + y;
}

int main(void)
{
    struct big g;
    struct odd o;
    int i;

    for (i = 0; i < 10; i++)
        g.a[i] = i + 1;
    for (i = 0; i < 41; i++)
        o.c[i] = (char)i;
    report(sum(g, 100));
    report(g.a[0]);
    report(again(1000, g, o, 7));
    return 0;
}
