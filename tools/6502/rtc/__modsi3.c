extern unsigned long __umodsi3 (unsigned long, unsigned long);
long __modsi3 (long a, long b)
{
  int neg = 0;
  if (a < 0) { a = -a; neg = 1; }
  if (b < 0) b = -b;
  a = __umodsi3 (a, b);
  return neg ? -a : a;
}
