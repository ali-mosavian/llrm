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
/* The arguments come from volatile objects: gcc evaluated the whole recursion at compile time (`pea 64`), so a clock census measured no
   kernel on its side. Every compiler runs the recursion. */
static volatile long n0 = 5, a0 = 1, b0 = 2, c0 = 3, d0 = 4;
int main(void) { report(deep(n0, a0, b0, c0, d0)); return 0; }
