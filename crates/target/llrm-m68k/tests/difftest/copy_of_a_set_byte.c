extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static void bump(struct S *p, unsigned char x) { p->a = (unsigned char)(p->a + x); p->b = (unsigned short)(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }
static unsigned char h0(void) {
  unsigned long v0 = (unsigned long)(2147483647UL + zero);
  unsigned long v1 = (unsigned long)(4294967295UL + zero);
  unsigned long v2 = (unsigned long)(1UL + zero);
  unsigned short v3 = (unsigned short)(0U + zero);
  unsigned short ck = 716 + zero;
  switch ((unsigned char)((unsigned short)v2 >= (unsigned short)((unsigned short)v2 >= 1U)) & 3) {
  case 1: ;
    v1 = (unsigned long)(v1 | (unsigned long)(unsigned long)((1UL < (unsigned long)buf[((unsigned char)((unsigned)(unsigned char)buf[((unsigned char)((unsigned)(unsigned char)v2 + (unsigned)(unsigned char)v0) & 15)] - (unsigned)255U) & 15)]) && ((unsigned char)v2 < 127U)));
  }
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v1 ^ (unsigned)(v1 >> 8);
  return (unsigned char)((unsigned char)(short)(unsigned short)(unsigned short)((unsigned)(unsigned short)v3 % ((unsigned)37212U | 1)) ^ (unsigned char)ck);
}
static unsigned long h1(unsigned long p0) {
  unsigned short v0 = (unsigned short)(1U + zero);
  unsigned short v1 = (unsigned short)(32767U + zero);
  unsigned short v2 = (unsigned short)(0U + zero);
  unsigned char a0[8];
  unsigned short a1[16];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 951 + zero;
  for (j = 0; j < 8; j++) a0[j] = (unsigned char)(j * 3U + 6 + zero);
  for (j = 0; j < 16; j++) a1[j] = (unsigned short)(j * 3U + 2 + zero);
  v1 = (unsigned short)(v1 ^ (unsigned)(unsigned short)h0());
  switch ((unsigned char)(!((unsigned short)v1 == (unsigned short)v0)) & 3) {
  }
  if ((((signed char)(unsigned char)buf[7] > (signed char)255U) || ((unsigned long)buf[((unsigned char)v2 & 15)] != (unsigned long)p0)) && ((short)(unsigned short)a1[((unsigned char)v2 & 15)] < (short)(unsigned short)((unsigned)(unsigned short)v1 * (unsigned)(unsigned short)v0))) {
    p0 = (unsigned long)((((short)(unsigned short)v0 > (short)(unsigned short)p0) || ((unsigned char)((unsigned)(unsigned char)p0 + (unsigned)(unsigned char)p0))) && ((unsigned char)((unsigned)0U - (unsigned)(unsigned char)p0) != (unsigned char)((unsigned)(unsigned char)p0 * (unsigned)(unsigned char)buf[((unsigned char)((unsigned)0U ^ (unsigned)(unsigned char)v0) & 15)])));
    switch (0U & 3) {
    }
  }
  ck = (ck << 1 | ck >> 15) ^ (unsigned)p0 ^ (unsigned)(p0 >> 8);
  return (unsigned long)((unsigned long)((unsigned long)(unsigned long)((unsigned long)3253533326UL >> 27) >> 8) ^ (unsigned long)ck);
}
static unsigned short h2(unsigned long p0, unsigned short p1) {
  unsigned char v0 = (unsigned char)(2U + zero);
  unsigned long v1 = (unsigned long)(3827027266UL + zero);
  unsigned char v2 = (unsigned char)(128U + zero);
  unsigned long a0[4];
  unsigned long a1[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 117 + zero;
  for (j = 0; j < 4; j++) a0[j] = (unsigned long)(j * 1U + 0 + zero);
  for (j = 0; j < 4; j++) a1[j] = (unsigned long)(j * 1U + 4 + zero);
  return (unsigned short)((unsigned short)v1 ^ (unsigned short)ck);
}
unsigned short test(void) {
  unsigned short v0 = (unsigned short)(0);
  unsigned long v1 = (unsigned long)(0);
  unsigned long v2 = (unsigned long)(0);
  unsigned short ck = 428 + zero;
  struct S s;
  s.a = 4 + zero; s.b = 11266 + zero; s.c = 218 + zero;
  switch (0) {
  }
  switch (0) {
  }
  v0 = (unsigned short)((unsigned)(unsigned short)(0) ^ (unsigned)(unsigned short)h1(1));
  return (unsigned short)((unsigned short)((unsigned)(unsigned short)v0 ^ (unsigned)(unsigned short)(0)) ^ (unsigned short)ck);
}
