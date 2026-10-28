// flags: -O0 -m32 | -O2 -m32
// Variadic long doubles at -m32: each argument takes a 12-byte slot of the stack (gcc's), and is not passed as a double (the
// convention's variadic_float promotion is for float).
extern void report(long value);

#ifdef __clang__
#include <stdarg.h>
/* clang copies a parameter whose address is taken: the arguments are read through its own va_list. */
long double sum(int n, ...)
{
    va_list ap;
    long double total = 0;
    int i;

    va_start(ap, n);
    for (i = 0; i < n; i++)
        total += va_arg(ap, long double);
    va_end(ap);
    return total;
}
#else
long double sum(int n, ...)
{
    char *ap = (char *)&n + 4;
    long double total = 0;
    int i;

    for (i = 0; i < n; i++) {
        total += *(long double *)ap;
        ap += 12;
    }
    return total;
}
#endif

int main(void)
{
    volatile long double a = 1.5L, b = 0.25L;

    report((long)(sum(3, a, b, a) * 4));
    return 0;
}
