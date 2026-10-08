/* strcmp and strncpy, which Borland C inlines and its library leaves out: the QCport modules llrm-c compiles call them.
   They are defined after Borland's declarations, which are cdecl: a definition has the convention of its prior declaration.
   Built with -O1 and no loop-idiom pass, which would turn these loops into calls to themselves. */
#include <string.h>
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
