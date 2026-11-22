// flags: -O2 -fno-inline-functions | -O1 -fno-inline-functions | -Os -fno-inline-functions
// A recursive function with 20 bytes of arguments and a call result added to a value: on the 6502 the register saves and a
// spill of the entry were stored before the frame was opened, into the caller's slots (the run HALTed at every level).
extern void report(long value);
static long deep(long n, long a, long b, long c, long d)
{
    long t, s;
    if (n == 0) {
        t = a+b+c;
        s = d;
    } else {
        t = deep(n - 1, b, c, d, a + n);
        s = n;
    }
    return t + s;
}
int main(void) { report(deep(5, 1, 2, 3, 4)); return 0; }
