// flags: -O2 | -O3
// targets: !ptr16
// Sums over run-time and constant trip counts of 16-bit data, widened to 64 bits and to 32: the vector loop covers 16 or 32
// iterations at a time and its remainder is widened in turn (a 4- or 8-lane loop), then a scalar tail. A remainder resumed from the
// wrong index, or a total not carried from one loop to the next, sums other elements. The oracle is a volatile loop, which is never
// vectorized. Prints 0, else the bad count then the first failing form and n.
extern void report(long value);

static short data[200];

__attribute__((noinline)) long sum64(const short *p, int n)
{
    long s = 0;
    for (int i = 0; i < n; i++) s += p[i];
    return s;
}

__attribute__((noinline)) int sum32(const short *p, int n)
{
    int s = 0;
    for (int i = 0; i < n; i++) s += p[i];
    return s;
}

__attribute__((noinline)) long sum40(const short *p)
{
    long s = 0;
    for (int i = 0; i < 40; i++) s += p[i];
    return s;
}

__attribute__((noinline)) long oracle64(const short *p, int n)
{
    volatile long s = 0;
    for (volatile int i = 0; i < n; i++) s = s + p[i];
    return s;
}

int main(void)
{
    long bad = 0, form = -1, first = -1;
    for (int i = 0; i < 200; i++) data[i] = (short)(i * 977 - 5000);
    for (int n = 0; n <= 150; n++) {
        long want = oracle64(data, n);
        if (sum64(data, n) != want) {
            if (!bad) form = 64, first = n;
            bad++;
        }
        if (sum32(data, n) != (int)want) {
            if (!bad) form = 32, first = n;
            bad++;
        }
    }
    if (sum40(data) != oracle64(data, 40)) {
        if (!bad) form = 40, first = 40;
        bad++;
    }
    report(bad);
    if (bad) {
        report(form);
        report(first);
    }
    return 0;
}
