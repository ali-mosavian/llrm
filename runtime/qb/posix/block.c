/* The block operations of qb.h on a host: the C library's. */
#include <string.h>

#include "qb.h"

void copy_bytes(char *to, const char *from, unsigned n)
{
    memmove(to, from, n);
}

void fill_bytes(char *to, int value, unsigned n)
{
    memset(to, value, n);
}

int compare_bytes(const char *a, const char *b, unsigned n)
{
    int order = memcmp(a, b, n);

    return order < 0 ? -1 : order > 0;
}
