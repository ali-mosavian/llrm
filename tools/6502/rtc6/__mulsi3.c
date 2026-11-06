unsigned long __mulsi3 (unsigned long a, unsigned long b)
{
  unsigned long r = 0;
  for (; b; b >>= 1, a <<= 1)
    if (b & 1) r += a;
  return r;
}
