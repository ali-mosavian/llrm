/* __ashrdi3: a count of 0..63, as __ashldi3. */
#include "i64.h"

i64 SFB_NAME(ashrdi3)(i64 x, int n)
{
    pair a = split((u64)x), r;
    int bm = 32 - n;
    if (!n) return x;
    if (bm <= 0) { r.hi = (uint32_t)((int32_t)a.hi >> 31); r.lo = (uint32_t)((int32_t)a.hi >> -bm); }
    else { r.hi = (uint32_t)((int32_t)a.hi >> n); r.lo = (a.lo >> n) | (a.hi << bm); }
    return (i64)join(r);
}
