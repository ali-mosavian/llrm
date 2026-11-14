// flags: -O2 | -O3
// targets: !ptr16
// Count-up and count-down copy loops over 8-, 16- and 32-bit elements, the pointer based on `i - 1` or on `i`, with source and
// destination overlapping either way. A vectorized count-down loop addressed each part from the stepped induction value, VF*IC-1
// elements too low: aarch64 -O2 and -O3 stored the wrong elements (8232 of 16200 byte-copy cases). The oracle is a volatile
// loop, which is never vectorized. Prints 0, else the bad count then the first failing form, n and offset.
extern void report(long value);

#define COPIES(SUF, T, Q)                                                                \
    __attribute__((noinline)) void up_##SUF(Q T *d, Q const T *s, long n)                \
    {                                                                                    \
        for (long i = 0; i < n; i++) d[i] = s[i];                                        \
    }                                                                                    \
    __attribute__((noinline)) void down1_##SUF(Q T *d, Q const T *s, long n)             \
    {                                                                                    \
        for (long i = n; i > 0; i--) d[i - 1] = s[i - 1];                                \
    }                                                                                    \
    __attribute__((noinline)) void downi_##SUF(Q T *d, Q const T *s, long n)             \
    {                                                                                    \
        for (long i = n; i > 0; i--) d[i] = s[i];                                        \
    }

COPIES(c, unsigned char, )
COPIES(s, unsigned short, )
COPIES(w, unsigned int, )
COPIES(vc, unsigned char, volatile)
COPIES(vs, unsigned short, volatile)
COPIES(vw, unsigned int, volatile)

#define SPAN 200
#define BASE 80
#define MAXN 72
#define MAXOFF 34

static long bad, first_form = -1, first_n, first_off;

#define SWEEP(FORM, T, FN, ORACLE)                                                       \
    do {                                                                                 \
        static T got[SPAN], want[SPAN];                                                  \
        for (long n = 0; n < MAXN; n++)                                                  \
            for (long off = -MAXOFF; off <= MAXOFF; off++) {                             \
                for (long i = 0; i < SPAN; i++) got[i] = want[i] = (T)(i * 7 + 3);       \
                FN(got + BASE + off, got + BASE, n);                                     \
                ORACLE(want + BASE + off, want + BASE, n);                               \
                long k = 0;                                                              \
                while (k < SPAN && got[k] == want[k]) k++;                               \
                if (k < SPAN) {                                                          \
                    if (!bad) first_form = FORM, first_n = n, first_off = off;           \
                    bad++;                                                               \
                }                                                                        \
            }                                                                            \
    } while (0)

int main(void)
{
    SWEEP(0, unsigned char, up_c, up_vc);
    SWEEP(1, unsigned char, down1_c, down1_vc);
    SWEEP(2, unsigned char, downi_c, downi_vc);
    SWEEP(3, unsigned short, up_s, up_vs);
    SWEEP(4, unsigned short, down1_s, down1_vs);
    SWEEP(5, unsigned short, downi_s, downi_vs);
    SWEEP(6, unsigned int, up_w, up_vw);
    SWEEP(7, unsigned int, down1_w, down1_vw);
    SWEEP(8, unsigned int, downi_w, downi_vw);
    report(bad);
    if (bad) {
        report(first_form);
        report(first_n);
        report(first_off);
    }
    return 0;
}
