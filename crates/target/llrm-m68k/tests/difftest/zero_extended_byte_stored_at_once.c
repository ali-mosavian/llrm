extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static void bump(struct S *p, unsigned char x) { p->a = (unsigned char)(p->a + x); p->b = (unsigned short)(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }
static unsigned long h0(unsigned long p0, unsigned short p1, unsigned short p2) {
  unsigned short v2 = (unsigned short)(0U + zero);
  unsigned long a0[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 855 + zero;
  for (j = 0; j < 4; j++) a0[j] = (unsigned long)(j * 7U + 0 + zero);
  switch ((unsigned char)p0 & 3) {
  }
  return (unsigned long)(2147483647UL ^ (unsigned long)ck);
}
static unsigned char h1(unsigned short p0, unsigned char p1) {
  unsigned char v2 = (unsigned char)(255U + zero);
  unsigned short ck = 489 + zero;
  return (unsigned char)(255U ^ (unsigned char)ck);
}
unsigned short test(void) {
  unsigned short v0 = (unsigned short)(0);
  unsigned short v1 = (unsigned short)(0);
  unsigned char v2 = (unsigned char)(0);
  unsigned short v3 = (unsigned short)(0);
  unsigned short v4 = (unsigned short)(0);
  unsigned long a0[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 400 + zero;
  struct S s;
  for (j = 0; j < 4; j++) a0[j] = (unsigned long)(0);
  s.a = 133 + zero; s.b = 25556 + zero; s.c = 19 + zero;
  if (0) {
  }
  for (i0 = 0; i0 < 3; i0++) {
    v4 = (unsigned short)(0);
    for (i1 = 2; i1; i1--) {
    }
  }
  switch (0) {
  }
  a0[(0)] = (unsigned long)((unsigned long)(unsigned long)(0) | (unsigned long)(unsigned long)buf[(0)]);
  for (j = 0; j < 4; j++) ck = (ck << 1 | ck >> 15) ^ (unsigned)a0[j];
  return (unsigned short)((unsigned short)(0) ^ (unsigned short)ck);
}
