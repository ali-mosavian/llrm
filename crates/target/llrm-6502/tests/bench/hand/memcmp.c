int k_memcmp(const void *a, const void *b, unsigned n)
{
    const unsigned char *p = a;
    const unsigned char *q = b;

    while (n--) {
        if (*p != *q)
            return *p < *q ? -1 : 1;
        ++p;
        ++q;
    }
    return 0;
}
