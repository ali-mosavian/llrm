/* __moddi3. */
#include "i64.h"

i64 SFB_NAME(moddi3)(i64 n, i64 d)
{
    pair a = split((u64)n), b = split((u64)d), r;
    int na = (int32_t)a.hi < 0, nb = (int32_t)b.hi < 0;
    udivmod(na ? neg(a) : a, nb ? neg(b) : b, &r);
    return (i64)join(na ? neg(r) : r);
}
