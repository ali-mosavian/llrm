// flags: -O1 -m32 | -O1
// 1500 levels of recursion kept at -O1: the flat target's stack held about 100 of them (DOS/32A exception 06h).
extern void report(long value);

static long mix(long a, long b, long c, long d, long e, int n)
{
    if (n == 0) return a + b + c + d + e;
    return mix(b + 1, c + a % 5, d ^ b, e + c % 3, a + d % 7, n - 1);
}

int main(void)
{
    report(mix(1, 2, 3, 4, 5, 1500));
    return 0;
}
