void *k_memchr(const void *p, int c, unsigned n)
{
    const unsigned char *q = p;

    while (n--) {
        if (*q == (unsigned char)c)
            return (void *)q;
        ++q;
    }
    return 0;
}
