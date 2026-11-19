#include "_di.h"
u64 __ashrdi3 (u64 x, unsigned char n)
{
  U u; u.v = x;
  u32 sign = (long) u.s.hi < 0 ? 0xFFFFFFFFUL : 0;
  if (n & 32) { u.s.lo = u.s.hi; u.s.hi = sign; n &= 31; }
  if (n) { u.s.lo = (u.s.lo >> n) | (u.s.hi << (32 - n)); u.s.hi = (u.s.hi >> n) | (sign << (32 - n)); }
  return u.v;
}
