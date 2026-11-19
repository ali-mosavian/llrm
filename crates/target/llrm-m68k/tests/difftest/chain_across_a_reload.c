extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static void bump(struct S *p, unsigned char x) { p->a = (unsigned char)(p->a + x); p->b = (unsigned short)(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }
static unsigned char h0(unsigned short p0, unsigned long p1, unsigned long p2) {
  unsigned short v3 = (unsigned short)(65535U + zero);
  unsigned char v4 = (unsigned char)(2U + zero);
  unsigned short ck = 182 + zero;
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v4 ^ (unsigned)(v4 >> 8);
  return (unsigned char)((unsigned char)buf[0] ^ (unsigned char)ck);
}
unsigned short test(void) {
  unsigned long v0 = (unsigned long)(0);
  unsigned long v1 = (unsigned long)(0);
  unsigned char v3 = (unsigned char)(0);
  unsigned char v4 = (unsigned char)(1);
  unsigned long a0[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 260 + zero;
  struct S s;
  for (j = 0; j < 4; j++) a0[j] = (unsigned long)(0);
  s.a = 87 + zero; s.b = 5549 + zero; s.c = 198 + zero;
  i0 = 2;
  while (i0) {
    i0--;
    switch ((unsigned char)((unsigned)(unsigned char)((unsigned char)((unsigned)64U + (unsigned)(unsigned char)v1)) + (unsigned)(unsigned char)(0)) & 3) {
    case 1: ;
      a0[2] = (unsigned long)((unsigned long)buf[11] < (unsigned long)((unsigned long)(unsigned long)a0[((unsigned char)h0((unsigned short)buf[(0)], (unsigned long)buf[(0)], (unsigned long)v0) & 3)] ^ (unsigned long)(unsigned long)a0[1]));
      bump(&s, (unsigned char)(1));
    }
    a0[(1)] = (unsigned long)a0[2];
    if (1) {
      v1 = (unsigned long)(v1 + (unsigned long)(unsigned long)v4);
      ck = (ck << 1 | ck >> 15) ^ (unsigned short)((unsigned)(unsigned short)a0[((unsigned char)(0 - (unsigned)(unsigned char)buf[2]) & 3)] << 6);
    }
    ck = (ck << 1 | ck >> 15) ^ (unsigned short)h0((unsigned short)buf[(0)], (unsigned long)v3, (unsigned long)v4);
    v4 = (unsigned char)(0);
  }
  switch (0) {
  }
  ck = (ck << 1 | ck >> 15) ^ s.a ^ s.b ^ s.c;
  return (unsigned short)((unsigned short)~(unsigned)(unsigned short)s.a ^ (unsigned short)ck);
}
