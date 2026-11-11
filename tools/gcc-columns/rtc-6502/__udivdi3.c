#include "_di.h"
u64 __udivmoddi4 (u64 a, u64 b, u64 *rem)
{
  U n, d, q, r; n.v = a; d.v = b; q.v = 0; r.v = 0;
  for (int i = 63; i >= 0; i--)
    {
      u32 bit = i >= 32 ? (n.s.hi >> (i - 32)) & 1 : (n.s.lo >> i) & 1;
      r.s.hi = (r.s.hi << 1) | (r.s.lo >> 31);
      r.s.lo = (r.s.lo << 1) | bit;
      if (r.s.hi > d.s.hi || (r.s.hi == d.s.hi && r.s.lo >= d.s.lo))
	{
	  u32 borrow = r.s.lo < d.s.lo;
	  r.s.lo -= d.s.lo; r.s.hi -= d.s.hi + borrow;
	  if (i >= 32) q.s.hi |= 1UL << (i - 32); else q.s.lo |= 1UL << i;
	}
    }
  if (rem) *rem = r.v;
  return q.v;
}
u64 __udivdi3 (u64 a, u64 b) { return __udivmoddi4 (a, b, 0); }
