extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static void bump(struct S *p, unsigned char x) { p->a = (unsigned char)(p->a + x); p->b = (unsigned short)(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }
static unsigned long h0(unsigned char p0, unsigned long p1) {
  unsigned short v0 = (unsigned short)(1U + zero);
  unsigned short v2 = (unsigned short)(65535U + zero);
  unsigned char v3 = (unsigned char)(127U + zero);
  unsigned char v4 = (unsigned char)(128U + zero);
  unsigned short a0[8];
  unsigned short a1[8];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 291 + zero;
  for (j = 0; j < 8; j++) a0[j] = (unsigned short)(j * 3U + 1 + zero);
  for (j = 0; j < 8; j++) a1[j] = (unsigned short)(j * 5U + 3 + zero);
  ck = (ck << 1 | ck >> 15) ^ (unsigned)p0 ^ (unsigned)(p0 >> 8);
  ck = (ck << 1 | ck >> 15) ^ (unsigned)p1 ^ (unsigned)(p1 >> 8);
  for (j = 0; j < 8; j++) ck = (ck << 1 | ck >> 15) ^ (unsigned)a0[j];
  for (j = 0; j < 8; j++) ck = (ck << 1 | ck >> 15) ^ (unsigned)a1[j];
  return (unsigned long)((unsigned long)(0 - (unsigned long)(unsigned long)((short)(unsigned short)(unsigned short)v2 >> 5)) ^ (unsigned long)ck);
}
unsigned short test(void) {
  unsigned char v0 = (unsigned char)(0U + zero);
  unsigned char v1 = (unsigned char)(127U + zero);
  unsigned short v2 = (unsigned short)(44730U + zero);
  unsigned char v3 = (unsigned char)(0);
  unsigned char a0[8];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 263 + zero;
  struct S s;
  for (j = 0; j < 8; j++) a0[j] = (unsigned char)(0);
  s.a = 110 + zero; s.b = 36808 + zero; s.c = 215 + zero;
  switch ((unsigned char)v0 & 3) {
  case 1: ;
    v2 = (unsigned short)(0);
    break;
  default: ;
    v0 = (unsigned char)(0);
  }
  ck = (ck << 1 | ck >> 15) ^ (unsigned short)(0 - (unsigned)(unsigned short)v0);
  v3 = (unsigned char)h0((unsigned char)(0), (unsigned long)~(unsigned long)(unsigned long)((unsigned long)(unsigned long)s.b - (unsigned long)(unsigned long)v0));
  v0 = (unsigned char)(1);
  v3 = (unsigned char)(0);
  for (i0 = 6; i0; i0--) {
    v3 = (unsigned char)(v3 | (unsigned)((!((unsigned char)((unsigned char)h0((unsigned char)a0[7], (unsigned long)v3)))) ? (unsigned char)a0[(0)] : (unsigned char)((unsigned char)i0)));
    if ((unsigned short)(short)(unsigned short)(unsigned short)a0[((unsigned char)((unsigned)(unsigned char)v2 * (unsigned)(unsigned char)v1) & 7)] >= (unsigned short)i0) {
      a0[7] = (unsigned char)h0((unsigned char)((unsigned)(unsigned char)v1 - (unsigned)(unsigned char)a0[((unsigned char)h0(166U, (unsigned long)i0) & 7)]), (unsigned long)h0((unsigned char)v2, 4294967295UL));
      ck = (ck << 1 | ck >> 15) ^ (unsigned short)((unsigned)(unsigned short)a0[(0)] % ((unsigned)(unsigned short)v2 | 1));
    }
    v1 = (unsigned char)(0);
    ck = (ck << 1 | ck >> 15) ^ (unsigned short)(0);
  }
  bump(&s, (unsigned char)v2);
  i0 = 6;
  while (i0) {
    i0--;
    switch (0) {
    }
    ck = (ck << 1 | ck >> 15) ^ (unsigned short)h0((unsigned char)i0, (unsigned long)a0[(0)]);
    for (i1 = 5; i1; i1--) {
      v2 = (unsigned short)(v2 ^ (unsigned)(((unsigned char)((unsigned)(unsigned char)v1 >> 1) < (unsigned char)h0((unsigned char)a0[(0)], (unsigned long)v3)) ? (unsigned short)(0) : (unsigned short)((unsigned)(unsigned short)i0 % ((unsigned)(unsigned short)v1 | 1))));
      ck = (ck << 1 | ck >> 15) ^ (unsigned short)v3;
      if (0) break;
    }
    ck = (ck << 1 | ck >> 15) ^ (unsigned short)((unsigned)(unsigned short)v2 - (unsigned)(unsigned short)v3);
  }
  return (unsigned short)((unsigned short)(0) ^ (unsigned short)ck);
}
