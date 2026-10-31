unsigned k_strnlen(const char *s, unsigned max)
{
    unsigned n = 0;

    while (n < max && s[n])
        ++n;
    return n;
}
