extern unsigned long __udivsi3 (unsigned long, unsigned long);
long __divsi3 (long a, long b)
{
  int neg = 0;
  if (a < 0) { a = -a; neg = 1; }
  if (b < 0) { b = -b; neg ^= 1; }
  a = __udivsi3 (a, b);
  return neg ? -a : a;
}
