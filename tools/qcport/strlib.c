/* strcmp and strncpy, which Borland C inlines and its library leaves out: the QCport modules llrm-c compiles call them.
   Built with -O1 and no loop-idiom pass, which would turn these loops into calls to themselves. */
int strcmp(const char *a, const char *b)
{
    while (*a && *a == *b) { a++; b++; }
    return (unsigned char)*a - (unsigned char)*b;
}

char *strncpy(char *d, const char *s, unsigned n)
{
    unsigned i;
    for (i = 0; i < n && s[i]; i++) d[i] = s[i];
    for (; i < n; i++) d[i] = 0;
    return d;
}
