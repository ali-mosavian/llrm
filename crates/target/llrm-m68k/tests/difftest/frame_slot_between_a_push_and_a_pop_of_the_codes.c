extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static void bump(struct S *p, unsigned char x) { p->a = (unsigned char)(p->a + x); p->b = (unsigned short)(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }
static unsigned long h0(unsigned short p0) {
  unsigned char v0 = (unsigned char)(255U + zero);
  unsigned char v1 = (unsigned char)(1U + zero);
  unsigned short ck = 954 + zero;
  p0 = (((unsigned char)(2147483647UL > 1768885974UL) != (unsigned char)((unsigned)(unsigned char)v1 | (unsigned)(unsigned char)p0)) ? (unsigned short)((unsigned)(unsigned short)((short)43317U > (short)(unsigned short)buf[((unsigned char)((unsigned)(unsigned char)v1 + (unsigned)(unsigned char)p0) & 15)]) | (unsigned)(unsigned short)((unsigned char)(0U <= (unsigned char)v1))) : (unsigned short)((unsigned short)((unsigned)14750U % ((unsigned)1U | 1)) > (unsigned short)((unsigned)65535U >> 6)));
  p0 = (unsigned short)((unsigned)65535U + (unsigned)(unsigned short)((unsigned)(unsigned short)((unsigned)(unsigned short)buf[((unsigned char)((unsigned)(unsigned char)v1 * (unsigned)179U) & 15)] - (unsigned)(unsigned short)v1) + (unsigned)(unsigned short)((unsigned)(unsigned short)buf[((unsigned char)((unsigned)255U / ((unsigned)127U | 1)) & 15)] + (unsigned)(unsigned short)buf[2])));
  p0 = (unsigned short)(p0 | (unsigned)(unsigned short)v0);
  ck = (ck << 1 | ck >> 15) ^ 54699U;
  ck = (ck << 1 | ck >> 15) ^ (unsigned)p0 ^ (unsigned)(p0 >> 8);
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v0 ^ (unsigned)(v0 >> 8);
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v1 ^ (unsigned)(v1 >> 8);
  return (unsigned long)((unsigned long)(!((unsigned char)((unsigned)127U ^ (unsigned)(unsigned char)v0))) ^ (unsigned long)ck);
}
static unsigned long h1(unsigned long p0, unsigned long p1) {
  unsigned char v0 = (unsigned char)(1U + zero);
  unsigned long v1 = (unsigned long)(2147483647UL + zero);
  unsigned short ck = 879 + zero;
  p0 = (unsigned long)(p0 ^ (unsigned long)(unsigned long)((unsigned long)(unsigned long)h0(1U) & (unsigned long)(unsigned long)((unsigned long)2369352226UL & (unsigned long)(unsigned long)v0)));
  switch ((unsigned char)h0((unsigned short)((unsigned)(unsigned short)p1 / ((unsigned)(unsigned short)p0 | 1))) & 3) {
  }
  ck = (ck << 1 | ck >> 15) ^ (unsigned)p0 ^ (unsigned)(p0 >> 8);
  return (unsigned long)((unsigned long)h0((unsigned short)((unsigned long)v1 <= (unsigned long)p1)) ^ (unsigned long)ck);
}
static unsigned long h2(void) {
  unsigned char v0 = (unsigned char)(128U + zero);
  unsigned short v1 = (unsigned short)(3272U + zero);
  unsigned char v2 = (unsigned char)(0U + zero);
  unsigned char a0[8];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 99 + zero;
  for (j = 0; j < 8; j++) a0[j] = (unsigned char)(j * 1U + 3 + zero);
  switch ((unsigned char)(0 - (unsigned)(unsigned char)h0((unsigned short)v0)) & 3) {
  }
  for (i0 = 2; i0; i0--) {
    if ((unsigned long)((unsigned char)v1 != (unsigned char)i0) <= (unsigned long)buf[((unsigned char)((unsigned)(unsigned char)buf[((unsigned char)((unsigned long)v2 < 2147483647UL) & 15)] ^ (unsigned)0U) & 15)]) {
      a0[2] = (unsigned char)((unsigned)(unsigned char)((unsigned short)buf[5] == (unsigned short)i0) << 14);
    }
  }
  return (unsigned long)((unsigned long)((unsigned long)(unsigned long)((unsigned long)(unsigned long)v2 >> 31) - (unsigned long)(unsigned long)((unsigned long)(unsigned long)v2 * (unsigned long)(unsigned long)v2)) ^ (unsigned long)ck);
}
unsigned short test(void) {
  unsigned short v0 = (unsigned short)(0);
  unsigned short v1 = (unsigned short)(0);
  unsigned short v2 = (unsigned short)(0);
  unsigned char a0[8];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 244 + zero;
  struct S s;
  for (j = 0; j < 8; j++) a0[j] = (unsigned char)(1);
  s.a = 31 + zero; s.b = 62029 + zero; s.c = 233 + zero;
  v1 = (unsigned short)(0);
  switch (0) {
  case 1: ;
    bump(&s, (unsigned char)(0));
  }
  i0 = 8;
  while (i0) {
    i0--;
    v0 = (0);
    ck = (ck << 1 | ck >> 15) ^ (unsigned short)h2();
    for (i1 = 0; i1 < 2; i1++) {
      a0[((unsigned char)((unsigned)(unsigned char)a0[7] & (unsigned)(unsigned char)i1) & 7)] = (unsigned char)((unsigned)((0) ? (unsigned char)v1 : (unsigned char)buf[((unsigned char)buf[11] & 15)]) % ((unsigned)(unsigned char)h0(0) | 1));
      v1 = (unsigned short)((unsigned)(unsigned short)(((unsigned char)((unsigned char)((unsigned char)h1((unsigned long)s.c, (unsigned long)v2)))) && ((unsigned char)((unsigned)127U * (unsigned)(unsigned char)i0))) & (unsigned)(unsigned short)(1));
      ck = (ck << 1 | ck >> 15) ^ (unsigned short)((unsigned)(unsigned short)v1 & (unsigned)32767U);
    }
    switch (0) {
    default: ;
      v2 = (unsigned short)(0);
      ck = (ck << 1 | ck >> 15) ^ (unsigned short)((unsigned)(unsigned short)a0[1] * (unsigned)(unsigned short)v1);
    }
  }
  return (unsigned short)((unsigned short)v0 ^ (unsigned short)ck);
}
