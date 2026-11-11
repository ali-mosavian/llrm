#include "_di.h"
u64 __lshrdi3 (u64 x, unsigned char n)
{
  U u; u.v = x;
  if (n & 32) { u.s.lo = u.s.hi; u.s.hi = 0; n &= 31; }
  if (n) { u.s.lo = (u.s.lo >> n) | (u.s.hi << (32 - n)); u.s.hi >>= n; }
  return u.v;
}
