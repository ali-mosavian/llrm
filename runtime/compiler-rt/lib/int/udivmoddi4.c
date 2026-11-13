/* __udivmoddi4: the quotient returned, the remainder stored through the third argument. */
#include "i64.h"

u64 SFB_NAME(udivmoddi4)(u64 n, u64 d, u64 *rem)
{
    pair r;
    pair q = udivmod(split(n), split(d), &r);
    *rem = join(r);
    return join(q);
}
