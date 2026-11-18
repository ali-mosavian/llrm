unsigned long sum_bytes_wide(const unsigned char *a)
{
    unsigned long s = 0;
    for (int i = 0; i < 20000; i++)
        s += a[i];
    return s;
}
