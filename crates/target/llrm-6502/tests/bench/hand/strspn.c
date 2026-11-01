unsigned k_strspn(const char *s, const char *set)
{
    unsigned n = 0;
    const char *p;
    for (; *s; s++, n++) {
        for (p = set; *p != *s; p++)
            if (*p == 0)
                return n;
    }
    return n;
}
