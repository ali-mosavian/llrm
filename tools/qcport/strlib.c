/* strcmp and strncpy, which Borland C inlines and its library leaves out: the QCport modules llrm-c compiles call them, as Borland's
   headers declare them, cdecl: a definition standing in for a library routine says so, or it is spelled for the default convention.
   Built with -O1 and no loop-idiom pass, which would turn these loops into calls to themselves. */
int __cdecl strcmp(const char *a, const char *b)
{
    while (*a && *a == *b) { a++; b++; }
    return (unsigned char)*a - (unsigned char)*b;
}

char * __cdecl strncpy(char *d, const char *s, unsigned n)
{
    unsigned i;
    for (i = 0; i < n && s[i]; i++) d[i] = s[i];
    for (; i < n; i++) d[i] = 0;
    return d;
}
