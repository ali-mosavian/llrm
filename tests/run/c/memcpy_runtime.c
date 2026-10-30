// flags: -O0 -m32 | -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* The runtime's memcpy (memcpy_ in the default convention, _memcpy in sysv): dst in EAX, src in EDX, n in EBX, or on the stack; dst comes
   back; every byte and no more is copied, n = 0 and n past 64 KiB included. Called through a pointer so that the call is the symbol, not
   an inline `rep movs`. */
extern void report(long value);
extern void *memcpy(void *dst, const void *src, unsigned long n);

static unsigned char from[70016], to[70016];
static void *(*volatile copy)(void *, const void *, unsigned long) = memcpy;
static unsigned long sizes[] = {0, 1, 3, 4, 5, 63, 64, 130, 70000};

int main(void)
{
    long failures = 0, total = 0;
    unsigned long i, k, n;

    for (i = 0; i < sizeof from; i++)
        from[i] = (unsigned char)(i * 7 + 3);
    for (k = 0; k < sizeof sizes / sizeof sizes[0]; k++) {
        n = sizes[k];
        for (i = 0; i < sizeof to; i++)
            to[i] = 0xAA;
        if (copy(to + 1, from + 2, n) != to + 1)
            failures++;
        if (to[0] != 0xAA || to[n + 1] != 0xAA)
            failures++;
        for (i = 0; i < n; i++)
            if (to[1 + i] != from[2 + i]) {
                failures++;
                break;
            }
        total += (long)n;
    }
    report(failures);
    report(total);
    return 0;
}
