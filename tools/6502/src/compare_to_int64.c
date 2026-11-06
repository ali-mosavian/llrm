// flags: -O0 | -O2 | -Os
// A comparison made a `long long` (gcc.c-torture 20080529-1, pr83383, 950512-1: "an i1 made i64"): the result of a compare
// and of `&&`, widened to 64 bits, and summed so each half is read.
extern void report(long value);

typedef long long s64;
typedef unsigned long long u64;

static s64 lt(int a, int b) { return a < b; }
static s64 ne(long a, long b) { return (s64)(a != b); }
static u64 both(int a, int b) { return (u64)(a && b) << 33; }

int main(void)
{
    s64 total = 0;
    int a, b;
    for (a = -1; a <= 1; ++a)
        for (b = -1; b <= 1; ++b)
            total = total * 3 + lt(a, b) + ne(a, b) * 2 + (s64)(both(a, b) >> 33) * 5 - (s64)(a == b);
    report((long)total);
    report((long)(total >> 3) + (long)(both(1, 1) >> 32));
    return 0;
}
