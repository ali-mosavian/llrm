int k_strnicmp(const char *s1, const char *s2, unsigned n)
{
    while (n--) {
        unsigned char a = *s1++, b = *s2++;
        if (a >= 'a' && a <= 'z')
            a -= 'a' - 'A';
        if (b >= 'a' && b <= 'z')
            b -= 'a' - 'A';
        if (a != b)
            return a < b ? -1 : 1;
        if (a == 0)
            break;
    }
    return 0;
}
