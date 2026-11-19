char *k_strstr(const char *hay, const char *needle)
{
    for (;; hay++) {
        const char *h = hay, *n = needle;
        while (*n && *h == *n) {
            h++;
            n++;
        }
        if (*n == 0)
            return (char *)hay;
        if (*hay == 0)
            return 0;
    }
}
