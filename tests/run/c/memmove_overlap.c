// flags: -O0 | -O2 -march=i486
/* Copy loops over overlapping bytes, either direction, which loop idiom recognition turns into a memmove of a variable length: under --isel=dag m16 calls the runtime's. */
extern void report(long v);
char buf[32];
static void up(unsigned n) { unsigned i; for (i = 0; i < n; ++i) buf[i] = buf[i + 3]; }
static void down(unsigned n) { unsigned i; for (i = n; i > 0; --i) buf[i + 2] = buf[i - 1]; }
static void fill(void) { int i; for (i = 0; i < 32; ++i) buf[i] = (char)(i + 1); }
static long sum(int from, int to) { long t = 0; int i; for (i = from; i < to; ++i) t = t * 3 + buf[i]; return t; }
int main(void)
{
    unsigned n = 10;
    fill();
    down(n);
    report(sum(0, 16));
    fill();
    up(n);
    report(sum(0, 16));
    fill();
    down(n + 17);
    report(sum(18, 32));
    return 0;
}
