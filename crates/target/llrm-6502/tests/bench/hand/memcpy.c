void *k_memcpy(void *d, const void *s, unsigned n)
{
    unsigned char *dp = d;
    const unsigned char *sp = s;

    while (n--)
        *dp++ = *sp++;
    return d;
}
