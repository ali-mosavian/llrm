// flags: -O1 | -O2
// A 64-bit function that pushes six registers had its 16-aligned array at [rsp+8]: the pad that rounds the frame sat above the locals, so the base lost the array's alignment and the `movaps` that zeroed it faulted (long_compare_chain -O1 segfault).
extern void report(long value);

__attribute__((noinline)) long f(long a, long b) { return a < b; }
static const long v[] = {0L, 1L, -1L, 5L, 7L, 9L, 11L, 13L};

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
    return 0;
}
