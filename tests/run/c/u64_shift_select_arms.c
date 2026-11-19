// flags: -O2 | -Os
/* A 64-bit value shifted left or right by a variable amount, chosen by a comparison: both arms call a shift routine
   with the value on the stack, and the call's stack reservation sits ahead of the branch. The right-shift arm stored
   its arguments 6 B below the reservation and returned 0. */
extern void report(long value);

typedef unsigned long long u64;
typedef unsigned long u32;

static u64 pick(u32 a, int e)
{
    u64 m = (a & 0x7FFFFFul) | 0x800000ul;
    return e >= 23 ? m << (e - 23) : m >> (23 - e);
}

int main(void)
{
    volatile u32 a = 0x40700000ul;
    volatile int e = 1;
    report((long)pick(a, e));
    e = 23;
    report((long)pick(a, e));
    e = 24;
    report((long)pick(a, e));
    e = 0;
    report((long)pick(a, e));
    return 0;
}
