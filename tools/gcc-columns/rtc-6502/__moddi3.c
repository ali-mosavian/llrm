#include "_di.h"
extern u64 __udivmoddi4 (u64, u64, u64 *);
static u64 neg (u64 x) { U u; u.v = x; u.s.lo = -u.s.lo; u.s.hi = ~u.s.hi + (u.s.lo == 0); return u.v; }
static int isneg (u64 x) { U u; u.v = x; return (long) u.s.hi < 0; }
long long __moddi3 (long long a, long long b)
{
  int n = 0; u64 x = a, y = b, r;
  if (isneg (x)) { x = neg (x); n = 1; }
  if (isneg (y)) y = neg (y);
  __udivmoddi4 (x, y, &r);
  return n ? neg (r) : r;
}
