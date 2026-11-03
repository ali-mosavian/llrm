extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static unsigned long h0(unsigned long p0) {
  unsigned long v0 = (unsigned long)(3311318042UL + zero);
  unsigned short a0[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 33 + zero;
  for (j = 0; j < 4; j++) a0[j] = (unsigned short)(j * 3U + 2 + zero);
  for (j = 0; j < 4; j++) ck = (ck << 1 | ck >> 15) ^ (unsigned)a0[j];
  return (unsigned long)((unsigned long)((unsigned long)(unsigned long)((signed char)(unsigned char)(unsigned char)v0 >> 3) / ((unsigned long)(unsigned long)((unsigned long)1UL & (unsigned long)2147483647UL) | 1)) ^ (unsigned long)ck);
}
static unsigned char h1(void) {
  unsigned long v1 = (unsigned long)(0UL + zero);
  unsigned long a0[16];
  unsigned long a1[16];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 510 + zero;
  for (j = 0; j < 16; j++) a0[j] = (unsigned long)(j * 9U + 0 + zero);
  for (j = 0; j < 16; j++) a1[j] = (unsigned long)(j * 5U + 0 + zero);
  return (unsigned char)((unsigned char)((0U) && ((unsigned char)((unsigned)(unsigned char)v1 / ((unsigned)(unsigned char)a0[((unsigned char)v1 & 15)] | 1)))) ^ (unsigned char)ck);
}
unsigned short test(void) {
  unsigned char v0 = (unsigned char)(0U + zero);
  unsigned char v1 = (unsigned char)(0);
  unsigned char v2 = (unsigned char)(0);
  unsigned long v3 = (unsigned long)(2147483647UL + zero);
  unsigned long v4 = (unsigned long)(0);
  unsigned short ck = 862 + zero;
  struct S s;
  s.a = 87 + zero; s.b = 38213 + zero; s.c = 47 + zero;
  switch (0) {
  case 0: ;
    v3 = (unsigned long)(v3 | (unsigned long)(unsigned long)(1));
    ck = (0) ^ (unsigned short)v0;
    v4 = (unsigned long)((unsigned long)2639022689UL - (unsigned long)(unsigned long)(0));
  }
  v2 = (unsigned char)((unsigned)(unsigned char)h0(0) + (unsigned)(unsigned char)(0));
  v0 = (unsigned char)~(unsigned)(0);
  ck = (ck << 1 | ck >> 15) ^ (((signed char)255U >= (signed char)(unsigned char)s.c) ? (unsigned short)v4 : 28654U);
  switch (0) {
  }
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v0 ^ (unsigned)(0);
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v1 ^ (unsigned)(0);
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v2 ^ (unsigned)(0);
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v3 ^ (unsigned)(v3 >> 8);
  ck = (ck << 1 | ck >> 15) ^ s.a ^ s.b ^ s.c;
  return (unsigned short)((unsigned short)(0) ^ (unsigned short)ck);
}
