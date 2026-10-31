/* The few C library routines the runtime's wasm32 build calls; the compiler lowers its own copies and fills
   to calls of these past the inline limit. Each is `no_builtin`, as the routines of compiler-rt are: a loop in
   it is never recognised as a call of the routine itself. */
#include <stddef.h>

__attribute__((no_builtin("memcpy")))
void *memcpy(void *to, const void *from, size_t count)
{
    unsigned char *d = to;
    const unsigned char *s = from;

    while (count--)
        *d++ = *s++;
    return to;
}

__attribute__((no_builtin("memmove")))
void *memmove(void *to, const void *from, size_t count)
{
    unsigned char *d = to;
    const unsigned char *s = from;

    if (d < s || d >= s + count)
        while (count--)
            *d++ = *s++;
    else
        while (count--)
            d[count] = s[count];
    return to;
}

__attribute__((no_builtin("memset")))
void *memset(void *to, int value, size_t count)
{
    unsigned char *d = to;

    while (count--)
        *d++ = (unsigned char)value;
    return to;
}

int memcmp(const void *a, const void *b, size_t count)
{
    const unsigned char *x = a, *y = b;

    for (; count; count--, x++, y++)
        if (*x != *y)
            return *x - *y;
    return 0;
}

size_t strlen(const char *text)
{
    const char *end = text;

    while (*end)
        end++;
    return (size_t)(end - text);
}

/* The generated code's `setjmp` of a function with ON ERROR: never a second return (see include/setjmp.h). */
int _setjmp(void *buffer)
{
    (void)buffer;
    return 0;
}

/* Nothing sleeps: the tick thread (runtime/qb/dev/host/tick.c) never starts on one thread. */
struct timespec;
int nanosleep(const struct timespec *wait, struct timespec *left)
{
    (void)wait;
    (void)left;
    return 0;
}
