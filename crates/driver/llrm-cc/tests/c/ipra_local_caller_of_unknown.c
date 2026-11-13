extern long unseen(long x);

__attribute__((noinline)) long wraps(long x) { return unseen(x) + 1; }

long keeps_in_saved(long a, long b, long c, long d, long e)
{
    return wraps(a) + wraps(b) + wraps(c) + wraps(d) + wraps(e);
}
