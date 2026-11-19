/* __muldi3. */
#include "i64.h"

u64 SFB_NAME(muldi3)(u64 x, u64 y)
{
    pair a = split(x), b = split(y), r;
    sf_mul32x32(a.lo, b.lo, &r.hi, &r.lo);
    r.hi += a.lo * b.hi + a.hi * b.lo;
    return join(r);
}
