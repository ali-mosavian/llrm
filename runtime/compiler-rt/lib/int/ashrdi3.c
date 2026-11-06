/* __ashrdi3: a count of 0..63, as __ashldi3. */
#include "i64.h"

i64 SFB_NAME(ashrdi3)(i64 x, int n)
{
    pair a = split((u64)x), r;
    uint32_t sign = (uint32_t)((int32_t)a.hi >> 31);
    if ((unsigned)n >= 32) { r.lo = (uint32_t)((int32_t)a.hi >> (n - 32)); r.hi = sign; }
    else if (n) { r.hi = (uint32_t)((int32_t)a.hi >> n); r.lo = (a.lo >> n) | (a.hi << (32 - n)); }
    else r = a;
    return (i64)join(r);
}
