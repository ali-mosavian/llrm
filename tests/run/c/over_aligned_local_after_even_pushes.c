// flags: -O1 | -O2
// A 64-bit function that pushes six registers had its 16-aligned array at [rsp+8]: the pad that rounds the frame sat above the locals, so the base lost the array's alignment and the `movaps` that zeroed it faulted (long_compare_chain -O1 segfault).
extern void report(long value);

__attribute__((noinline)) long f(long a, long b) { return a < b; }
static const long v[] = {0L, 1L, -1L, 5L, 7L, 9L, 11L, 13L};

/* Control: the same array in frames of five and of six pushed registers; the address must be 16-aligned in both. */
#define LIVE(name, n, ...) \
    __attribute__((noinline)) long name(long a, long b, long c, long d, long e, long f) \
    { \
        long m[8] __attribute__((aligned(16))); \
        long x = f_(a), y = f_(b), z = f_(c), u = f_(d), w = f_(e), t = f_(f); \
        for (int j = 0; j < 8; ++j) m[j] = 0; \
        g_(m); \
        return (long)((unsigned long)m & 15) + x + y + z + u + w + (n > 5 ? t : 0); \
    }
__attribute__((noinline)) long f_(long x) { return x * 3; }
__attribute__((noinline)) void g_(long *p) { p[1] = 1; }
LIVE(five, 5)
LIVE(six, 6)

int main(void)
{
    unsigned i, j;
    long m[8] __attribute__((aligned(16)));

    for (i = 0; i < 8; ++i) {
        for (j = 0; j < 8; ++j)
            m[j] = 0;
        for (j = 0; j < 8; ++j) {
            long a = v[i], b = v[j];
            m[0] |= f(a, b) << j; m[1] |= f(b, a) << j; m[2] |= f(a, a) << j; m[3] |= f(b, b) << j;
            m[4] |= f(a, b + 1) << j; m[5] |= f(a + 1, b) << j; m[6] |= f(b, a + 1) << j; m[7] |= f(b + 1, a) << j;
        }
        for (j = 0; j < 8; ++j)
            report(m[j]);
        report((long)((unsigned long)m & 15));
    }
    report(five(1, 2, 3, 4, 5, 6));
    report(six(1, 2, 3, 4, 5, 6));
    return 0;
}
