/* Folds to upper case, as the hand routine does: '_' sorts after "a". */
int k_stricmp(const char *a, const char *b)
{
    const unsigned char *p = (const unsigned char *)a;
    const unsigned char *q = (const unsigned char *)b;
    unsigned char c, d;

    for (;;) {
        c = *p++;
        d = *q++;
        if (c >= 'a' && c <= 'z')
            c -= 'a' - 'A';
        if (d >= 'a' && d <= 'z')
            d -= 'a' - 'A';
        if (c != d)
            return c < d ? -1 : 1;
        if (c == 0)
            return 0;
    }
}
