extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static unsigned long h0(void) {
  unsigned char v0 = (unsigned char)(16U + zero);
  unsigned long v1 = (unsigned long)(109301446UL + zero);
  unsigned long v2 = (unsigned long)(0UL + zero);
  unsigned short v3 = (unsigned short)(0U + zero);
  unsigned long a0[16];
  unsigned short a1[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 136 + zero;
  for (j = 0; j < 16; j++) a0[j] = (unsigned long)(j * 7U + 4 + zero);
  for (j = 0; j < 4; j++) a1[j] = (unsigned short)(j * 1U + 6 + zero);
  v0 = (unsigned char)(v0 | (unsigned)(unsigned char)(short)(unsigned short)(unsigned short)((unsigned)(unsigned short)a1[((unsigned char)((unsigned)(unsigned char)a0[((unsigned char)((unsigned)(unsigned char)v3 * (unsigned)41U) & 15)] * (unsigned)129U) & 3)] / ((unsigned)(unsigned short)v2 | 1)));
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v0 ^ (unsigned)(v0 >> 8);
  return (unsigned long)((((unsigned long)((unsigned long)(unsigned long)v3 * (unsigned long)(unsigned long)v3) < (unsigned long)((unsigned long)3000596052UL | (unsigned long)(unsigned long)v3)) ? (unsigned long)((unsigned char)v0 > (unsigned char)v3) : (unsigned long)((unsigned long)(unsigned long)v3 >> 18)) ^ (unsigned long)ck);
}
static unsigned char h1(unsigned long p0, unsigned long p1, unsigned char p2) {
  unsigned char v0 = (unsigned char)(1U + zero);
  unsigned char v1 = (unsigned char)(128U + zero);
  unsigned short v2 = (unsigned short)(9166U + zero);
  unsigned long a0[8];
  unsigned short a1[8];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 108 + zero;
  for (j = 0; j < 8; j++) a0[j] = (unsigned long)(j * 1U + 4 + zero);
  for (j = 0; j < 8; j++) a1[j] = (unsigned short)(j * 7U + 5 + zero);
  if ((unsigned short)a1[((unsigned char)((signed char)127U > (signed char)(unsigned char)a1[((unsigned char)v1 & 7)]) & 7)] != (unsigned short)((unsigned)(unsigned short)p0 + (unsigned)32767U)) {
  }
  return (unsigned char)((unsigned char)h0() ^ (unsigned char)ck);
}
unsigned short test(void) {
  unsigned short v0 = (unsigned short)(0);
  unsigned char v1 = (unsigned char)(0);
  unsigned long v2 = (unsigned long)(0);
  unsigned short ck = 679 + zero;
  struct S s;
  s.a = 45 + zero; s.b = 2553 + zero; s.c = 170 + zero;
  if (0) {
    switch (0) {
    }
  }
  ck = (0) ^ (unsigned short)h0();
  return (unsigned short)((0) ^ (unsigned short)ck);
}
