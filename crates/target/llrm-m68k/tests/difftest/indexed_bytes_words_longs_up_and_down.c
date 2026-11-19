extern const unsigned char buf[16];
extern const volatile unsigned char zero;
unsigned short test(void) {
  unsigned char b[8]; unsigned short w[8]; unsigned long l[8];
  unsigned char *pb = b; unsigned short *pw = w; unsigned long *pl = l;
  const unsigned char *s = buf;
  int i; unsigned long sum = zero;
  for (i = 0; i < 8; i++) { *pb++ = (unsigned char)(s[i] + 1); *pw++ = (unsigned short)(s[i] * 257u); *pl++ = s[i] * 65793ul; }
  for (i = 0; i < 8; i++) { sum += *--pb; sum += *--pw; sum += *--pl; }
  pb = b; pw = w; pl = l;
  for (i = 0; i < 8; i++) { sum = sum * 3 + *pb++; sum ^= *pw++; sum += *pl++; }
  return (unsigned short)(sum ^ (sum >> 16));
}
