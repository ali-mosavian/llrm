char *k_strrchr(const char *s, int c)
{
    const char *last = 0;
    for (;; s++) {
        if (*s == (char)c)
            last = s;
        if (*s == 0)
            return (char *)last;
    }
}
