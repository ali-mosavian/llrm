// flags: -O2 | -Os
// bar: none (SDCC does not halt)
/* A 64-bit value shifted left, logically right and arithmetically right by a count that is not a constant, for counts
   below, at and across whole bytes: the shift is one loop over eight bytes of the frame (a bit a trip), not a call. */
extern void report(long value);

typedef unsigned long long u64;
typedef long long s64;

static void both(u64 v)
{
    report((long)v);
    report((long)(v >> 32));
}

int main(void)
{
    volatile u64 x = 0x923456789ABCDEF1ull;
    volatile int n;
    n = 0;
    both(x << n);
    both(x >> n);
    both((u64)((s64)x >> n));
    n = 1;
    both(x << n);
    both(x >> n);
    both((u64)((s64)x >> n));
    n = 7;
    both(x << n);
    both(x >> n);
    both((u64)((s64)x >> n));
    n = 8;
    both(x << n);
    both(x >> n);
    both((u64)((s64)x >> n));
    n = 9;
    both(x << n);
    both(x >> n);
    both((u64)((s64)x >> n));
    n = 24;
    both(x << n);
    both(x >> n);
    both((u64)((s64)x >> n));
    n = 33;
    both(x << n);
    both(x >> n);
    both((u64)((s64)x >> n));
    n = 63;
    both(x << n);
    both(x >> n);
    both((u64)((s64)x >> n));
    return 0;
}
