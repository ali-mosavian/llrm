long from1(const int *a, const int *b, int n)
{
    long total = 0;
    int i;
    for (i = 1; i <= n; i++)
        total += (long)a[i] * b[i - 1];
    return total;
}
