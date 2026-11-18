int log2_of(unsigned x)
{
    int n = 0;
    while (x >>= 1)
        n++;
    return n;
}
