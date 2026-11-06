/* Shifts: libgcc's (value, int count) signatures (__ashldi3). A count of 64 or more gives 0. */
#include "i64.h"

u64 SFB_NAME(ashldi3)(u64 x, int n)
{
    pair a = split(x), r = { 0, 0 };
    if ((unsigned)n >= 64) return 0;
    if (n >= 32) r.hi = a.lo << (n - 32);
    else if (n) { r.lo = a.lo << n; r.hi = (a.hi << n) | (a.lo >> (32 - n)); }
    else r = a;
    return join(r);
}
