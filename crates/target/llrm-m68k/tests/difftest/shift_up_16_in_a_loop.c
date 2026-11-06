extern const unsigned char buf[16];
extern const volatile unsigned char zero;
static unsigned short h0(unsigned short p0, unsigned char p1, unsigned long p2) {
  unsigned char v1 = (unsigned char)(127U + zero);
  unsigned long v2 = (unsigned long)(1048576UL + zero);
  unsigned char a0[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 203 + zero;
  for (j = 0; j < 4; j++) a0[j] = (unsigned char)(j * 9U + 2 + zero);
  for (j = 0; j < 4; j++) ck = (ck << 1 | ck >> 15) ^ (unsigned)a0[j];
  return (unsigned short)((unsigned short)((unsigned)((1U >= 0U) ? 65535U : (unsigned short)p0) | (unsigned)(unsigned short)((unsigned short)p1 > (unsigned short)a0[((unsigned char)((unsigned)(unsigned char)v2 << 1) & 3)])) ^ (unsigned short)ck);
}
static unsigned long h1(unsigned long p0, unsigned short p1, unsigned char p2) {
  unsigned short v0 = (unsigned short)(65535U + zero);
  unsigned short a0[4];
  unsigned char a1[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 226 + zero;
  for (j = 0; j < 4; j++) a0[j] = (unsigned short)(j * 1U + 2 + zero);
  for (j = 0; j < 4; j++) a1[j] = (unsigned char)(j * 7U + 6 + zero);
  v0 = (unsigned short)(v0 + (unsigned)(unsigned short)h0((unsigned short)h0((unsigned short)p2, 145U, (unsigned long)a1[2]), (unsigned char)(0 - (unsigned)(unsigned char)buf[14]), (unsigned long)((unsigned long)(unsigned long)buf[((unsigned char)((unsigned)(unsigned char)p1 ^ (unsigned)(unsigned char)p0) & 15)] - (unsigned long)(unsigned long)v0)));
  for (i0 = 4; i0; i0--) {
    p0 = (unsigned long)((unsigned long)(unsigned long)~(unsigned long)(unsigned long)((unsigned long)(unsigned long)p0 * (unsigned long)(unsigned long)p1) << 16);
    ck = (ck << 1 | ck >> 15) ^ (unsigned short)h0((unsigned short)i0, (unsigned char)buf[7], (unsigned long)p2);
  }
  return (unsigned long)((unsigned long)((unsigned long)(unsigned long)((unsigned long)(unsigned long)p0 - (unsigned long)1UL) + (unsigned long)1UL) ^ (unsigned long)ck);
}
unsigned short test(void){ unsigned long a=h1(3,0,0); return (unsigned short)h1(0,45880,0); }
