// RUN: llrm-cc %s -m64 -O1 -S -o /dev/stdout
// A phi of `i`, `1 - i`, `i - 2`, ... met after one of its operands made `16 - i` a second counter in the 12-trip loop:
// `sub rsp, 16` and a callee-saved push for it (div_constants_ulong -O1 4334 B against 4173 B).
// CHECK-LABEL: c3 proc near
// CHECK-NOT: sub rsp
// CHECK-NOT: push rbx
// CHECK: ret
static unsigned long state = 2463534242UL;
static unsigned long next(void) { state ^= state << 13; state ^= state >> 17; state ^= state << 5; return state; }
static unsigned long sum;
long c3(void)
{
    volatile unsigned long dv = 3;
    unsigned long x, q, r;
    long bad = 0, i;
    for (i = 0; i < 1500; ++i) {
        unsigned long w = next(), shape = next() & 7;
        x = shape == 0 ? w : shape == 1 ? w >> 16 : shape == 2 ? w >> 24 : shape == 3 ? w >> 29 : w;
        q = x / 3; r = x % 3;
        if (q != x / dv || r != x % dv) ++bad;
        sum += q * 31UL + r;
    }
    for (i = 0; i < 12; ++i) {
        unsigned long hi = 0xFFFFFFFFFFFFFFFFUL;
        x = i < 2 ? i : i < 4 ? hi - (i - 2) : i < 6 ? 3 + (i - 5) : i < 8 ? (hi / 3 * 3) - (i - 6) : i < 10 ? (hi - 3 + (i - 8)) : (2 * 3 - (i - 10));
        q = x / 3; r = x % 3;
        if (q != x / dv || r != x % dv) ++bad;
        sum += q * 31UL + r;
    }
    return bad;
}
