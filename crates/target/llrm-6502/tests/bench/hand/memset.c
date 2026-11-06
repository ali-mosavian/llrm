void *k_memset(void *p, int c, unsigned n)
{
    unsigned char *q = p;

    while (n--)
        *q++ = (unsigned char)c;
    return p;
}
