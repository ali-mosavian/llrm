extern long forward(long n, __builtin_va_list ap);

long sum_longs(short n, ...)
{
    __builtin_va_list ap;
    long total = 0;

    __builtin_va_start(ap, n);
    for (short i = 0; i < n; i++)
        total += __builtin_va_arg(ap, long) * (i + 1);
    __builtin_va_end(ap);
    return total;
}

double sum_doubles(short n, ...)
{
    __builtin_va_list ap;
    double total = 0;

    __builtin_va_start(ap, n);
    for (short i = 0; i < n; i++)
        total += __builtin_va_arg(ap, double);
    __builtin_va_end(ap);
    return total;
}

long passes_the_list(long n, ...)
{
    __builtin_va_list ap;
    long total;

    __builtin_va_start(ap, n);
    total = forward(n, ap);
    __builtin_va_end(ap);
    return total;
}
