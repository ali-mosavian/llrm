__attribute__((noinline)) long leaf(long x) { return x * 3; }

long keeps_in_scratch(long a, long b, long c, long d, long e)
{
    return leaf(a) + leaf(b) + leaf(c) + leaf(d) + leaf(e);
}
