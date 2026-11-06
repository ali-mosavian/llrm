extern unsigned int __umodhi3 (unsigned int, unsigned int);
int __modhi3 (int a, int b)
{
  int neg = 0;
  if (a < 0) { a = -a; neg = 1; }
  if (b < 0) b = -b;
  a = __umodhi3 (a, b);
  return neg ? -a : a;
}
