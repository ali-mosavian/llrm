/* Shift-subtract on operands aligned by their top bits; 16-bit operands take the short path. */
unsigned long __udivmodsi4 (unsigned long n, unsigned long d, unsigned long *r)
{
  unsigned long q = 0;
  int s = 0;
  if (!d) { *r = n; return ~0UL; }
  if (d > n) { *r = n; return 0; }
  if (!((n | d) >> 16)) { *r = (unsigned short) n % (unsigned short) d; return (unsigned short) n / (unsigned short) d; }
  while (!(d & 0x80000000UL) && d < n) { d <<= 1; s++; }
  for (;; s--)
    {
      q <<= 1;
      if (n >= d) { n -= d; q |= 1; }
      if (!s) break;
      d >>= 1;
    }
  *r = n;
  return q;
}
unsigned long __udivsi3 (unsigned long n, unsigned long d) { unsigned long r; return __udivmodsi4 (n, d, &r); }
