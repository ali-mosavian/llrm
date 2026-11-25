struct S { long a, b, c; long *p; };
long get(struct S *s, long d)
{
    if (d == s->a + s->b - 8)
        s->b -= 8;
    return s->p[d & 7];
}
long f(struct S *s, long n)
{
    long t = 0;
    for (long i = 0; i < n; i++)
        t += get(s, i) + get(s, i + 1);
    return t;
}
long g(struct S *s, long n) { return get(s, n) + get(s, n * 3); }
