char *k_strpbrk(const char *s, const char *set)
{
    const char *p;
    for (; *s; s++)
        for (p = set; *p; p++)
            if (*p == *s)
                return (char *)s;
    return 0;
}
