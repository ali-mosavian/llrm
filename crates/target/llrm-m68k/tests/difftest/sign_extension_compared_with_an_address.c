extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static void bump(struct S *p, unsigned char x) { p->a = (unsigned char)(p->a + x); p->b = (unsigned short)(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }
static unsigned char h0(void) {
  unsigned short v1 = (unsigned short)(8192U + zero);
  unsigned char v2 = (unsigned char)(127U + zero);
  unsigned short ck = 419 + zero;
  return (unsigned char)((unsigned char)v1 ^ (unsigned char)ck);
}
static unsigned short h1(unsigned short p0, unsigned short p1, unsigned short p2) {
  unsigned char a0[8];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 707 + zero;
  for (j = 0; j < 8; j++) a0[j] = (unsigned char)(j * 7U + 0 + zero);
  ck = (ck << 1 | ck >> 15) ^ (unsigned)p1 ^ (unsigned)(p1 >> 8);
  return (unsigned short)((unsigned short)h0() ^ (unsigned short)ck);
}
static unsigned short h2(unsigned short p0, unsigned long p1, unsigned char p2) {
  unsigned long v1 = (unsigned long)(0UL + zero);
  unsigned long v3 = (unsigned long)(3200288920UL + zero);
  unsigned char a0[8];
  unsigned long a1[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 132 + zero;
  for (j = 0; j < 8; j++) a0[j] = (unsigned char)(j * 1U + 4 + zero);
  for (j = 0; j < 4; j++) a1[j] = (unsigned long)(j * 5U + 3 + zero);
  return (unsigned short)((unsigned short)(signed char)(unsigned char)(unsigned char)((unsigned)(unsigned char)a1[((unsigned char)((unsigned)2U * (unsigned)127U) & 3)] & (unsigned)(unsigned char)p1) ^ (unsigned short)ck);
}
unsigned short test(void) {
  unsigned short v0 = (unsigned short)(65535U + zero);
  unsigned long v1 = (unsigned long)(0);
  unsigned char v2 = (unsigned char)(0);
  unsigned char a0[4];
  unsigned long a1[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 639 + zero;
  struct S s;
  for (j = 0; j < 4; j++) a0[j] = (unsigned char)(j * 7U + 6 + zero);
  for (j = 0; j < 4; j++) a1[j] = (unsigned long)(0);
  s.a = 193 + zero; s.b = 54566 + zero; s.c = 12 + zero;
  for (i0 = 8; i0; i0--) {
    if (0) {
    }
    for (i1 = 0; i1 < 8; i1++) {
      bump(&s, (unsigned char)((unsigned)(unsigned char)((unsigned)(unsigned char)v1 - (unsigned)(unsigned char)v0) - (unsigned)(unsigned char)(0)));
    }
    v1 = (unsigned long)((unsigned long)(unsigned long)h1((unsigned short)(0), (unsigned short)((short)(unsigned short)v0 < (short)(unsigned short)a0[0]), (unsigned short)~(unsigned)(unsigned short)v2) - (unsigned long)(unsigned long)(0));
  }
  while (0) {
  }
  ck = (0) ^ s.a ^ s.b ^ s.c;
  return (unsigned short)((unsigned short)(0) ^ (unsigned short)ck);
}
