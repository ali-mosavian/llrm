/* __divmoddi4: the quotient returned, the remainder (of the dividend's sign) stored through the third argument. */
#include "i64.h"

i64 SFB_NAME(divmoddi4)(i64 n, i64 d, i64 *rem)
{
    pair a = split((u64)n), b = split((u64)d), r;
    int na = (int32_t)a.hi < 0, nb = (int32_t)b.hi < 0;
    pair q = udivmod(na ? neg(a) : a, nb ? neg(b) : b, &r);
    *rem = (i64)join(na ? neg(r) : r);
    return (i64)join((na != nb) && (b.lo | b.hi) ? neg(q) : q);
}
