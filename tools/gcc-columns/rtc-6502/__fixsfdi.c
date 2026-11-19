#include "_di.h"
long long __fixsfdi (float f)
{
  union { float f; u32 u; } v; v.f = f;
  int e = (int) ((v.u >> 23) & 255) - 127;
  U r; r.v = 0;
  if (e < 0) return 0;
  u32 m = (v.u & 0x7FFFFFUL) | 0x800000UL;
  if (e >= 63) { r.s.hi = 0x7FFFFFFFUL; r.s.lo = 0xFFFFFFFFUL; if (v.u >> 31) { r.s.hi = 0x80000000UL; r.s.lo = 0; } return r.v; }
  if (e < 23) r.s.lo = m >> (23 - e);
  else
    {
      int k = e - 23; r.s.lo = m; r.s.hi = 0;
      while (k--) { r.s.hi = (r.s.hi << 1) | (r.s.lo >> 31); r.s.lo <<= 1; }
    }
  if (v.u >> 31) { r.s.lo = -r.s.lo; r.s.hi = ~r.s.hi + (r.s.lo == 0); }
  return r.v;
}
