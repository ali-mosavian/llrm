// flags: -fno-inline-functions
// Mutual recursion f -> h -> g -> h -> f: f and g repeat the call of h and the 32-bit arithmetic after it, which the module outlines as a
// called routine. A body that held the call of h would take two more bytes of the 256-byte return stack at every level, not once at the
// deepest. 61 is the deepest the stack takes with the outliner (62 without it); the answers are the host's.
extern void report(long value);

#define STEP(r, n) \
    unsigned long a = (r) + (n); \
    unsigned long c = a < (n); \
    unsigned long b = (a ^ ((n) << 3)) + ((r) >> 5); \
    unsigned long d = b - (n); \
    unsigned long e = d + (c << 16) + (a >> 9); \
    return e ^ (b + c);

static unsigned long h(unsigned long n);

static unsigned long f(unsigned long n)
{
    unsigned long r = h(n);
    { STEP(r, n) }
}

static unsigned long g(unsigned long n)
{
    unsigned long r = h(n);
    { STEP(r, n) }
}

static unsigned long h(unsigned long n)
{
    unsigned long r;
    if (n == 0)
        return 7;
    r = (n & 1) ? f(n - 1) : g(n - 1);
    return r + n;
}

int main(void)
{
    volatile unsigned long depth = 61;
    report((long)f(depth));
    report((long)g(depth - 1));
    return 0;
}
