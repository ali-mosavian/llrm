unsigned bytes(const unsigned char *a, const unsigned char *b, int n)
{
    unsigned total = 0;
    int i;
    for (i = 0; i < n; i++)
        total += a[i] & b[i];
    return total;
}
