/* __divmoddi4: the quotient returned, the remainder (of the dividend's sign) stored through the third argument.
   The signs are fixed here; the division is __udivmoddi4's, so a program that links both pays the core once. */
#include "i64.h"

u64 SFB_NAME(udivmoddi4)(u64 n, u64 d, u64 *rem);

i64 SFB_NAME(divmoddi4)(i64 n, i64 d, i64 *rem)
{
    pair a = split((u64)n), b = split((u64)d);
    u64 rr;
    int na = (int32_t)a.hi < 0, nb = (int32_t)b.hi < 0;
    pair q = split(SFB_NAME(udivmoddi4)(join(na ? neg(a) : a), join(nb ? neg(b) : b), &rr)), r = split(rr);
    *rem = (i64)join(na ? neg(r) : r);
    return (i64)join((na != nb) && (b.lo | b.hi) ? neg(q) : q);
}
