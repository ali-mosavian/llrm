// A divide in a loop that calls a local leaf: the divide's zero high half must be made at each divide, not held in a register.
static unsigned long state = 2463534242UL;
static unsigned long next(void) { state ^= state << 13; state ^= state >> 17; state ^= state << 5; return state; }
static unsigned long sum;
long g(void)
{
    volatile unsigned long dv = 3;
    long bad = 0, i;
    for (i = 0; i < 1500; ++i) {
        unsigned long w = next(), q, r;
        q = w / 3; r = w % 3;
        if (q != w / dv || r != w % dv) ++bad;
        sum += q * 31UL + r;
    }
    return bad;
}
