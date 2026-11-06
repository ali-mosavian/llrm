/* __lshrdi3: a count of 0..63, as __ashldi3. */
#include "i64.h"

u64 SFB_NAME(lshrdi3)(u64 x, int n)
{
    pair a = split(x), r;
    int bm = 32 - n;
    if (!n) return x;
    if (bm <= 0) { r.hi = 0; r.lo = a.hi >> -bm; }
    else { r.hi = a.hi >> n; r.lo = (a.lo >> n) | (a.hi << bm); }
    return join(r);
}
