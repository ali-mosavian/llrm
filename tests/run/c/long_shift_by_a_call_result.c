// flags: -O0 | -O2 | -Os
// A long shifted by a variable amount between two calls, in a loop that keeps two accumulators: the amount was read in
// place from its pair and never loaded into the register the shift routine takes it in, so the shift ran by what the
// register held (the first accumulator came out 2 where the host says 68).
extern void report(long value);

static int __attribute__((noinline)) slt(long a, long b) { return a < b; }
static int __attribute__((noinline)) sge(long a, long b) { return a >= b; }

static const long v[] = {0L, 1L, -1L, 0x7FFFL};

int main(void)
{
    unsigned i, j;
    long m[2];

    for (i = 0; i < 2; ++i) {
        m[0] = m[1] = 0;
        for (j = 0; j < 4; ++j) {
            long a = v[i], b = v[j];
            m[0] |= (long)slt(a, b) << (2 * j);
            m[1] |= (long)sge(a, b) << (2 * j);
        }
        report(m[0]);
        report(m[1]);
    }
    return 0;
}
