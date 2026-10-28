/* The C library routines the torture programs call, over the OS layer: what a program that checks itself needs to run. */
#include "llrm_os.h"

typedef unsigned size_t_;

void exit(int code) { llrm_os_exit((unsigned char)code); for (;;) {} }
void abort(void) { llrm_os_exit(134); for (;;) {} }

/* The target's runtime defines these (its c.toml `libcalls`); torture.py passes -DRUNTIME_HAS_NAME for each. */
#ifndef RUNTIME_HAS_MEMCPY
void *memcpy(void *to, const void *from, size_t_ n)
{
    unsigned char *d = to;
    const unsigned char *s = from;
    while (n--) *d++ = *s++;
    return to;
}
#endif

#ifndef RUNTIME_HAS_MEMMOVE
void *memmove(void *to, const void *from, size_t_ n)
{
    unsigned char *d = to;
    const unsigned char *s = from;
    if (d < s) while (n--) *d++ = *s++;
    else { size_t_ i = n; while (i) { --i; d[i] = s[i]; } }  /* not `*--d = *--s`: #811 */
    return to;
}
#endif

#ifndef RUNTIME_HAS_MEMSET
void *memset(void *to, int c, size_t_ n)
{
    unsigned char *d = to;
    while (n--) *d++ = (unsigned char)c;
    return to;
}
#endif

int memcmp(const void *a, const void *b, size_t_ n)
{
    const unsigned char *x = a, *y = b;
    for (; n; --n, ++x, ++y)
        if (*x != *y) return *x - *y;
    return 0;
}

size_t_ strlen(const char *s) { const char *e = s; while (*e) ++e; return (size_t_)(e - s); }

int strcmp(const char *a, const char *b)
{
    while (*a && *a == *b) { ++a; ++b; }
    return (unsigned char)*a - (unsigned char)*b;
}

int strncmp(const char *a, const char *b, size_t_ n)
{
    for (; n; --n, ++a, ++b) {
        if (*a != *b) return (unsigned char)*a - (unsigned char)*b;
        if (!*a) return 0;
    }
    return 0;
}

char *strcpy(char *to, const char *from) { char *d = to; while ((*d++ = *from++)) {} return to; }

char *strncpy(char *to, const char *from, size_t_ n)
{
    size_t_ i = 0;
    for (; i < n && from[i]; ++i) to[i] = from[i];
    for (; i < n; ++i) to[i] = 0;
    return to;
}

char *strcat(char *to, const char *from) { strcpy(to + strlen(to), from); return to; }

char *strchr(const char *s, int c)
{
    for (;; ++s) {
        if (*s == (char)c) return (char *)s;
        if (!*s) return 0;
    }
}

char *strrchr(const char *s, int c)
{
    const char *found = 0;
    for (;; ++s) {
        if (*s == (char)c) found = s;
        if (!*s) return (char *)found;
    }
}

int abs(int x) { return x < 0 ? -x : x; }
long labs(long x) { return x < 0 ? -x : x; }

/* A bump allocator over the OS layer's heap: nothing is returned. */
void *malloc(size_t_ n)
{
    unsigned char *p = llrm_os_more((n + 7u) & ~7u);
    return p;
}
void *calloc(size_t_ n, size_t_ size) { void *p = malloc(n * size); if (p) memset(p, 0, n * size); return p; }
void free(void *p) { (void)p; }
void *realloc(void *p, size_t_ n) { void *q = malloc(n); if (q && p) memcpy(q, p, n); return q; }

