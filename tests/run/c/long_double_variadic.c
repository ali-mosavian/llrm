// flags: -O0 -m32 | -O2 -m32
// Variadic long doubles at -m32: each argument takes a 12-byte slot of the stack (gcc's), and is not passed as a double (the
// convention's variadic_float promotion is for float).
extern void report(long value);

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

int main(void)
{
    volatile long double a = 1.5L, b = 0.25L;

    report((long)(sum(3, a, b, a) * 4));
    return 0;
}
