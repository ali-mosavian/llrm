unsigned k_strcspn(const char *s, const char *set)
{
    unsigned n = 0;
    const char *p;
    for (; *s; s++, n++) {
        for (p = set; *p; p++)
            if (*p == *s)
                return n;
    }
    return n;
}
