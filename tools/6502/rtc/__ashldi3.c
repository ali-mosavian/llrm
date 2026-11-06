#include "_di.h"
u64 __ashldi3 (u64 x, unsigned char n)
{
  U u; u.v = x;
  if (n & 32) { u.s.hi = u.s.lo; u.s.lo = 0; n &= 31; }
  if (n) { u.s.hi = (u.s.hi << n) | (u.s.lo >> (32 - n)); u.s.lo <<= n; }
  return u.v;
}
