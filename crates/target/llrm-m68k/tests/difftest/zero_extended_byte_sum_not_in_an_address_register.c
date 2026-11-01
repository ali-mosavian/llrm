extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static unsigned short h0(unsigned long p0) {
  unsigned char v0 = (unsigned char)(0U + zero);
  unsigned char v1 = (unsigned char)(127U + zero);
  unsigned char v2 = (unsigned char)(1U + zero);
  unsigned short a0[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 182 + zero;
  for (j = 0; j < 4; j++) a0[j] = (unsigned short)(j * 5U + 1 + zero);
  return (unsigned short)((unsigned short)((unsigned)(unsigned short)(0 - (unsigned)(unsigned short)v0) / ((unsigned)(unsigned short)((unsigned)(unsigned short)a0[2] * (unsigned)(unsigned short)v1) | 1)) ^ (unsigned short)ck);
}
static unsigned long h1(unsigned short p0) {
  unsigned long v0 = (unsigned long)(4294967295UL + zero);
  unsigned long v1 = (unsigned long)(268435456UL + zero);
  unsigned long v2 = (unsigned long)(1UL + zero);
  unsigned long a0[16];
  unsigned long a1[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 109 + zero;
  for (j = 0; j < 16; j++) a0[j] = (unsigned long)(j * 1U + 2 + zero);
  for (j = 0; j < 4; j++) a1[j] = (unsigned long)(j * 3U + 1 + zero);
  v2 = (unsigned long)((unsigned long)(unsigned long)((unsigned long)(unsigned long)((unsigned long)(unsigned long)v2 + (unsigned long)(unsigned long)buf[13]) * (unsigned long)(unsigned long)(1U >= (unsigned char)a1[((unsigned char)((unsigned char)v1 != (unsigned char)v2) & 3)])) << 11);
  if ((unsigned long)((unsigned long)(unsigned long)a1[((unsigned char)((unsigned)(unsigned char)p0 | (unsigned)(unsigned char)p0) & 3)] - (unsigned long)2147483647UL) == (unsigned long)((unsigned long)2147483647UL - (unsigned long)(unsigned long)v2)) {
    for (i0 = 0; i0 < 2; i0++) {
    }
    ck = (ck << 1 | ck >> 15) ^ (unsigned short)((signed char)(unsigned char)v2 < (signed char)(unsigned char)v2);
  }
  return (unsigned long)((unsigned long)buf[((unsigned char)((unsigned)(unsigned char)v1 - (unsigned)(unsigned char)v2) & 15)] ^ (unsigned long)ck);
}
unsigned short test(void) {
  unsigned short v0 = (unsigned short)(0);
  unsigned long v2 = (unsigned long)(0);
  unsigned char v3 = (unsigned char)(0);
  unsigned short a0[8];
  unsigned short a1[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 698 + zero;
  struct S s;
  for (j = 0; j < 8; j++) a0[j] = (unsigned short)(0);
  for (j = 0; j < 4; j++) a1[j] = (unsigned short)(0);
  s.a = 153 + zero; s.b = 9504 + zero; s.c = 106 + zero;
  switch ((unsigned char)h1(0) & 3) {
  case 0: ;
    v2 = (unsigned long)(v2 | (unsigned long)(unsigned long)((unsigned long)(0) < (unsigned long)h1(0)));
  }
  return (unsigned short)(0);
}
