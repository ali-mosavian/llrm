char *k_strncpy(char *dst, const char *src, unsigned n)
{
    char *d = dst;
    while (n && (*d = *src) != 0) {
        d++;
        src++;
        n--;
    }
    while (n) {
        *d++ = 0;
        n--;
    }
    return dst;
}
