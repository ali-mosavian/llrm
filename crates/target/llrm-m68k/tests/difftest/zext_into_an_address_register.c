extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static unsigned long h0(unsigned long p0) {
  unsigned long v2 = (unsigned long)(512UL + zero);
  unsigned short a0[4];
  unsigned char a1[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 689 + zero;
  for (j = 0; j < 4; j++) a0[j] = (unsigned short)(j * 9U + 6 + zero);
  for (j = 0; j < 4; j++) a1[j] = (unsigned char)(j * 1U + 3 + zero);
  return (unsigned long)((unsigned long)(!((unsigned long)v2 != (unsigned long)a0[((unsigned char)(0 - (unsigned)(unsigned char)p0) & 3)])) ^ (unsigned long)ck);
}
unsigned short test(void) {
  unsigned short v0 = (unsigned short)(0);
  unsigned short v1 = (unsigned short)(0);
  unsigned short a0[8];
  unsigned long a1[16];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 535 + zero;
  struct S s;
  for (j = 0; j < 8; j++) a0[j] = (unsigned short)(1);
  for (j = 0; j < 16; j++) a1[j] = (unsigned long)(0);
  s.a = 87 + zero; s.b = 10319 + zero; s.c = 223 + zero;
  for (i0 = 2; i0; i0--) {
  }
  for (i0 = 0; i0 < 9; i0++) {
    v1 = (unsigned short)((unsigned)(unsigned short)((unsigned)(unsigned short)((unsigned)(unsigned short)v0 * (unsigned)(unsigned short)i0) % ((unsigned)(unsigned short)v0 | 1)) ^ (unsigned)(unsigned short)(0));
    i1 = 4;
    while (i1) {
      i1--;
      v0 = (unsigned short)(0 - (unsigned)(unsigned short)((unsigned)(unsigned short)h0(0) & (unsigned)(unsigned short)h0(0)));
      ck = (0) ^ (unsigned short)((unsigned)(unsigned short)v1 % ((unsigned)(unsigned short)i0 | 1));
    }
  }
  return (unsigned short)ck;
}
