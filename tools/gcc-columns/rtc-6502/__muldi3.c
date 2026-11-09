typedef unsigned long u32;
typedef unsigned long long u64;
typedef union { u64 v; struct { u32 lo, hi; } s; } U;
extern u32 __mulsi3 (u32, u32);
u64 __muldi3 (u64 a, u64 b)
{
  U x, y, r; x.v = a; y.v = b;
  /* 32x32 -> 64 by 16-bit pieces, plus the cross terms */
  u32 al = x.s.lo & 0xFFFF, ah = x.s.lo >> 16, bl = y.s.lo & 0xFFFF, bh = y.s.lo >> 16;
  u32 ll = al * bl, lh = al * bh, hl = ah * bl, hh = ah * bh;
  u32 mid = (ll >> 16) + (lh & 0xFFFF) + (hl & 0xFFFF);
  r.s.lo = (ll & 0xFFFF) | (mid << 16);
  r.s.hi = hh + (lh >> 16) + (hl >> 16) + (mid >> 16) + x.s.lo * y.s.hi + x.s.hi * y.s.lo;
  return r.v;
}
