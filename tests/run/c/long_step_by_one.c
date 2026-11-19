// flags: -O0 | -O2 | -Os | -O2 -m32 | -Os -m32
/* A long grown or shrunk by one carries into its high word when the low word wraps: the carry is a zero test of the low
   word, and the step on the high word runs only then. */
extern void report(long value);

long inc(long x) { return x + 1; }
long dec(long x) { return x - 1; }
long add_ones(long x) { return x + 1 + 1; }

int main(void)
{
    static const long values[] = {0L, 1L, 65534L, 65535L, 65536L, 131071L, -1L, -2L, -65536L, -65537L, 2147483645L};
    for (int i = 0; i < 11; i++) {
        report(inc(values[i]));
        report(dec(values[i]));
        report(add_ones(values[i]));
    }
    return 0;
}
