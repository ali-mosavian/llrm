void *memcpy (void *d, const void *s, unsigned int n)
{
  unsigned char *p = d; const unsigned char *q = s;
  while (n--) *p++ = *q++;
  return d;
}
