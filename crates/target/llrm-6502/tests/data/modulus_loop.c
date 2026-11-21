long sum(const short *p, int n)
{
    long total = 0;
    int i;
    for (i = 0; i < n; ++i)
        total = (total * 3 + p[i]) % 1000003;
    return total;
}
