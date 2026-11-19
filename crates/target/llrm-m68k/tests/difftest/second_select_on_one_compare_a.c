extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static void bump(struct S *p, unsigned char x) { p->a = (unsigned char)(p->a + x); p->b = (unsigned short)(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }
static unsigned long h0(unsigned short p0, unsigned char p1) {
  unsigned char v0 = (unsigned char)(127U + zero);
  unsigned char v1 = (unsigned char)(0U + zero);
  unsigned long v2 = (unsigned long)(16384UL + zero);
  unsigned char v3 = (unsigned char)(127U + zero);
  unsigned char a0[8];
  unsigned char a1[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 990 + zero;
  for (j = 0; j < 8; j++) a0[j] = (unsigned char)(j * 1U + 0 + zero);
  for (j = 0; j < 4; j++) a1[j] = (unsigned char)(j * 7U + 2 + zero);
  switch ((unsigned char)((unsigned)(unsigned char)((unsigned)(unsigned char)v1 | (unsigned)(unsigned char)a1[3]) / ((unsigned)(unsigned char)p1 | 1)) & 3) {
  case 0: ;
    v0 = (unsigned char)((unsigned)(unsigned char)((unsigned)(unsigned char)((unsigned)(unsigned char)v3 & (unsigned)(unsigned char)v1) & (unsigned)(unsigned char)((unsigned)(unsigned char)a1[((unsigned char)((signed char)(unsigned char)(unsigned char)v0 >> 7) & 3)] & (unsigned)(unsigned char)p0)) ^ (unsigned)(unsigned char)((unsigned)(unsigned char)~(unsigned)255U + (unsigned)(((signed char)(unsigned char)v0 > (signed char)0U) ? (unsigned char)p0 : (unsigned char)p1)));
  }
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v0 ^ (unsigned)(v0 >> 8);
  return (unsigned long)((unsigned long)((unsigned long)(unsigned long)((unsigned long)(unsigned long)v1 & (unsigned long)(unsigned long)buf[7]) << 3) ^ (unsigned long)ck);
}
static unsigned short h1(unsigned long p0, unsigned long p1, unsigned char p2) {
  unsigned short v1 = (unsigned short)(65535U + zero);
  unsigned short v2 = (unsigned short)(23654U + zero);
  unsigned short ck = 452 + zero;
  switch ((unsigned char)v1 & 3) {
  }
  return (unsigned short)((unsigned short)p1 ^ (unsigned short)ck);
}
unsigned short test(void) {
  unsigned char v0 = (unsigned char)(0);
  unsigned long v1 = (unsigned long)(0);
  unsigned short v2 = (unsigned short)(0U + zero);
  unsigned long v3 = (unsigned long)(0);
  unsigned short ck = 51 + zero;
  struct S s;
  s.a = 127 + zero; s.b = 53633 + zero; s.c = 98 + zero;
  if (0) {
  }
  bump(&s, (unsigned char)h0((unsigned short)(1), (unsigned char)((unsigned)(unsigned char)v2 ^ (unsigned)(unsigned char)v0)));
  switch (0) {
  }
  ck = (0) ^ s.a ^ s.b ^ s.c;
  return (unsigned short)((unsigned short)(0) ^ (unsigned short)ck);
}
