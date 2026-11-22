extern int a(int), b(int), c(int), d(int);

static int big(int x)
{
    int r = a(x);
    r += b(r);
    r += c(r) * 3;
    if (r > 100)
        r -= d(r);
    return r + a(r) + b(r) + c(x);
}

int run(int k, int v)
{
    if (__builtin_expect(k == 3, 0))
        return big(v);
    return a(v);
}
