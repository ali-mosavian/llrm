// flags: -O0 -m32 | -O2 -march=i486 -m32 | -O2 -march=i486 -m32 -mabi=sysv
/* The runtime's memmove (memmove_, _memmove) and memset (memset_, _memset) against byte loops written here: overlaps in both
   directions at every offset from 1 to 5, sizes around the dword boundary, a size past 64 KiB, unaligned starts, no byte touched
   outside the range, dst back, and the direction flag clear at the return (a string operation after the call would run backwards).
   Called through pointers so that the call is the symbol, not an inline `rep movs`. */
extern void report(long value);
extern void *memmove(void *dst, const void *src, unsigned long n);
extern void *memset(void *dst, int c, unsigned long n);

static unsigned char got[70100], want[70100];
static void *(*volatile move)(void *, const void *, unsigned long) = memmove;
static void *(*volatile fill)(void *, int, unsigned long) = memset;
static unsigned long sizes[] = {0, 1, 2, 3, 4, 5, 7, 8, 9, 63, 64, 65, 129, 70000};

static void reference_move(unsigned char *dst, const unsigned char *src, unsigned long n)
{
    unsigned long i;
    if (dst <= src || dst >= src + n)
        for (i = 0; i < n; i++)
            dst[i] = src[i];
    else
        for (i = n; i > 0; i--)
            dst[i - 1] = src[i - 1];
}

static void pattern(void)
{
    unsigned long i;
    for (i = 0; i < sizeof got; i++)
        got[i] = want[i] = (unsigned char)(i * 13 + 5);
}

static long same(void)
{
    unsigned long i;
    for (i = 0; i < sizeof got; i++)
        if (got[i] != want[i])
            return 0;
    return 1;
}

int main(void)
{
    long failures = 0, cases = 0;
    unsigned long k, shift, dst_at, src_at, n;
    int up;

    for (k = 0; k < sizeof sizes / sizeof sizes[0]; k++) {
        n = sizes[k];
        for (up = 0; up < 2; up++)
            for (shift = 0; shift <= 5; shift++) {
                dst_at = up ? 8 + shift : 8;
                src_at = up ? 8 : 8 + shift;
                pattern();
                reference_move(want + dst_at, want + src_at, n);
                if (move(got + dst_at, got + src_at, n) != got + dst_at || !same())
                    failures++;
                cases++;
            }
        pattern();
        for (shift = 0; shift < 4; shift++) {
            unsigned long i;
            for (i = 0; i < n; i++)
                want[9 + shift + i] = (unsigned char)(0x5A + shift);
            if (fill(got + 9 + shift, 0x15A + (int)shift, n) != got + 9 + shift || !same())
                failures++;
            cases++;
            pattern();
        }
    }
    report(failures);
    report(cases);
    return 0;
}
