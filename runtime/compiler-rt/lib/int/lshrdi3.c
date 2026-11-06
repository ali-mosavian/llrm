/* __lshrdi3: count of 64 or more gives 0. */
#include "i64.h"

u64 SFB_NAME(lshrdi3)(u64 x, int n)
{
    pair a = split(x), r = { 0, 0 };
    if ((unsigned)n >= 64) return 0;
    if (n >= 32) r.lo = a.hi >> (n - 32);
    else if (n) { r.hi = a.hi >> n; r.lo = (a.lo >> n) | (a.hi << (32 - n)); }
    else r = a;
    return join(r);
}
