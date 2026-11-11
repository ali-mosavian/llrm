unsigned long __lshrsi3 (unsigned long x, unsigned char n)
{
  if (n & 16) { x >>= 16; n &= 15; }
  while (n--) x >>= 1;
  return x;
}
