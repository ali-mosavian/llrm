extern long add4(long a, long b, long c, long d);

long mix(long a, long b, long c)
{
    long r = add4(a, b, c, a + b);
    long s = add4(r, c, a, b);
    return r ^ (s + c - a + b - r) ^ (c & s & r);
}
