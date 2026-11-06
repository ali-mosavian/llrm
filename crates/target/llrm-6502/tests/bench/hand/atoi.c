/* Like cc65's atoi: only ' ' and '\t' are skipped; the value wraps at 16 bits. */
int k_atoi(const char *s)
{
    unsigned v = 0;
    int neg = 0;
    while (*s == ' ' || *s == '\t')
        ++s;
    if (*s == '-') {
        neg = 1;
        ++s;
    } else if (*s == '+') {
        ++s;
    }
    while (*s >= '0' && *s <= '9') {
        v = v * 10 + (*s - '0');
        ++s;
    }
    return (int)(neg ? 0u - v : v);
}
