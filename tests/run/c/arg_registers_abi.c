// flags: -O2 -fno-inline-functions | -O1 -fno-inline-functions | -Os -fno-inline-functions
// Where an argument travels: a function private to the module takes up to 18 bytes in a, x, rc2..rc7, rc16..rc25 and the rest
// on the stack; one another module can call (external, or its address taken) keeps 8 bytes in registers. Both kinds, called
// directly and through a pointer, with a callee-saved value held across the calls, a recursion, a struct by value and a variadic.
extern void report(long value);

static long f5(long a, long b, short c, signed char d, long e)
{
    return a * 3 + b * 5 + c * 7 + d * 11 + e * 13;
}

static long f6(long a, long b, long c, long d, long e, long f)
{
    return a + b * 2 + c * 3 + d * 4 + e * 5 + f * 6;
}

static long f7(long a, short b, long c, short d, long e, short f, long g)
{
    return a - b + c - d + e - f + g;
}

long ext3(long a, long b, long c)
{
    return a * 100 + b * 10 + c;
}

static long taken(long a, long b, long c)
{
    return a * 7 + b * 5 + c * 3;
}

long (*volatile through)(long, long, long) = ext3;
long (*volatile through_static)(long, long, long) = taken;

static long deep(long n, long a, long b, long c, long d, long e)
{
    if (n == 0)
        return a + b + c + d + e;
    return deep(n - 1, b, c, d, e, a + n) + n;
}

struct pair { long x, y; };
struct wide { long v[5]; };

static long by_pair(struct pair p, long z, long w)
{
    return p.x * 3 + p.y * 5 + z * 7 + w * 11;
}

static long by_wide(long first, struct wide w, long last)
{
    return first + w.v[0] + w.v[1] * 2 + w.v[2] * 3 + w.v[3] * 4 + w.v[4] * 5 + last;
}

static long sum_va(short n, ...)
{
    __builtin_va_list ap;
    long total = 0;
    short i;

    __builtin_va_start(ap, n);
    for (i = 0; i < n; i++)
        total += __builtin_va_arg(ap, long) * (i + 1);
    __builtin_va_end(ap);
    return total;
}

static long keeps(long a, long b)
{
    long held1 = a * 3, held2 = b * 5, held3 = a ^ b;
    long mid = f6(a, b, held1, held2, held3, a + b);

    return held1 + held2 + held3 + mid + f5(held3, held2, 3, 4, held1);
}

int main(void)
{
    long x = 12345, y = 678;

    report(f5(1, 2, 3, 4, 5));
    report(f5(x, y, -3, -4, 100000));
    report(f6(1, 2, 3, 4, 5, 6));
    report(f6(x, y, 1000, 2000, 3000, 4000));
    report(f7(1000, 2, 30000, 4, 500000, 6, 70));
    report(ext3(1, 2, 3));
    report(through(4, 5, 6));
    report(through_static(7, 8, 9));
    report(deep(5, 1, 2, 3, 4, 5));
    {
        struct pair p = { 40, 50 };
        struct wide w = { { 1, 2, 3, 4, 5 } };

        report(by_pair(p, 6, 7));
        report(by_wide(100, w, 200));
    }
    report(sum_va(5, 10L, 20L, 30L, 40L, 50L));
    report(sum_va(2, 1000000L, 3L));
    report(keeps(x, y));
    return 0;
}
