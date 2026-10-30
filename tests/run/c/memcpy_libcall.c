// flags: -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* Copies the compiler hands to the runtime's memcpy where it has one: over 128 bytes, of a variable length, of unaligned pointers.
   Every byte and no more arrives, and what the copy leaves in registers is not read afterwards. */
extern void report(long value);

struct big { int a; char pad[196]; int z; };
static struct big src, dst;
static unsigned char from[300], to[300];
static unsigned n_var = 150;
static unsigned off = 1;

static long same(const unsigned char *a, const unsigned char *b, unsigned n)
{
    unsigned i;
    for (i = 0; i < n; i++)
        if (a[i] != b[i])
            return 1;
    return 0;
}

int main(void)
{
    long failures = 0;
    unsigned i;

    for (i = 0; i < sizeof from; i++)
        from[i] = (unsigned char)(i * 5 + 1);
    src.a = 7;
    src.z = 9;
    for (i = 0; i < sizeof src.pad; i++)
        src.pad[i] = (char)(i + 3);
    dst = src;
    failures += same((unsigned char *)&src, (unsigned char *)&dst, sizeof src);
    failures += dst.a != 7 || dst.z != 9;

    for (i = 0; i < sizeof to; i++)
        to[i] = 0xAA;
    __builtin_memcpy(to + off, from + 2 * off, n_var);
    failures += to[0] != 0xAA || to[off + n_var] != 0xAA;
    failures += same(to + off, from + 2 * off, n_var);

    report(failures);
    report(to[off + n_var - 1]);
    return 0;
}
