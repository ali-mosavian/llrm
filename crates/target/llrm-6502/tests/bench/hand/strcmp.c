int k_strcmp(const char *a, const char *b)
{
    const unsigned char *p = (const unsigned char *)a;
    const unsigned char *q = (const unsigned char *)b;

    while (*p == *q) {
        if (*p == 0)
            return 0;
        ++p;
        ++q;
    }
    return *p < *q ? -1 : 1;
}
