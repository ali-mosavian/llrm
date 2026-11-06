extern unsigned int __udivhi3 (unsigned int, unsigned int);
int __divhi3 (int a, int b)
{
  int neg = 0;
  if (a < 0) { a = -a; neg = 1; }
  if (b < 0) { b = -b; neg ^= 1; }
  a = __udivhi3 (a, b);
  return neg ? -a : a;
}
