extern long add4(long a, long b, long c, long d);

long mix(long a, long b, long c)
{
    long t[4];

    t[0] = a;
    t[1] = b;
    t[2] = c;
    t[3] = add4(a, b, c, a + b);
    return t[0] * 3 + t[1] * 5 + t[2] * 7 + t[3];
}
