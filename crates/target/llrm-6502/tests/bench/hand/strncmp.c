int k_strncmp(const char *a, const char *b, unsigned n)
{
    const unsigned char *p = (const unsigned char *)a;
    const unsigned char *q = (const unsigned char *)b;

    while (n--) {
        if (*p != *q)
            return *p < *q ? -1 : 1;
        if (*p == 0)
            return 0;
        ++p;
        ++q;
    }
    return 0;
}
