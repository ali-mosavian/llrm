extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static unsigned char h0(void) {
  unsigned short v0 = (unsigned short)(32767U + zero);
  unsigned short v1 = (unsigned short)(65535U + zero);
  unsigned short v2 = (unsigned short)(65535U + zero);
  unsigned char v3 = (unsigned char)(1U + zero);
  unsigned long a0[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 968 + zero;
  for (j = 0; j < 4; j++) a0[j] = (unsigned long)(j * 3U + 1 + zero);
  if ((unsigned char)(signed char)(unsigned char)(unsigned char)a0[((unsigned char)((unsigned char)(128UL == 1UL)) & 3)] >= (unsigned char)(0 - (unsigned)(unsigned char)v1)) {
    if ((unsigned char)((unsigned)(unsigned char)v1 >> 10) <= (unsigned char)((unsigned)0U & (unsigned)(unsigned char)a0[1])) {
    }
    for (i0 = 0; i0 < 9; i0++) {
      v3 = (unsigned char)(v3 ^ (unsigned)(unsigned char)((unsigned short)((short)(unsigned short)(unsigned short)buf[((unsigned char)(8U > (unsigned char)v1) & 15)] >> 4) < (unsigned short)((unsigned char)v1 >= (unsigned char)v1)));
      ck = (ck << 1 | ck >> 15) ^ (unsigned short)(short)(unsigned short)(unsigned short)a0[((unsigned char)((unsigned)(unsigned char)i0 * (unsigned)(unsigned char)buf[((unsigned char)((unsigned)(unsigned char)v3 + (unsigned)(unsigned char)v2) & 15)]) & 3)];
    }
  }
  return (unsigned char)((unsigned char)(((unsigned char)v3 < 1U) && ((unsigned char)(short)(unsigned short)32767U)) ^ (unsigned char)ck);
}
unsigned short test(void) {
  unsigned char v0 = (unsigned char)(1U + zero);
  unsigned char v1 = (unsigned char)(0);
  unsigned long v2 = (unsigned long)(0UL + zero);
  unsigned long v3 = (unsigned long)(0);
  unsigned short v4 = (unsigned short)(57927U + zero);
  unsigned long a0[16];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 379 + zero;
  struct S s;
  for (j = 0; j < 16; j++) a0[j] = (unsigned long)(0);
  s.a = 117 + zero; s.b = 23143 + zero; s.c = 19 + zero;
  v1 = (unsigned char)((unsigned long)(1) < (unsigned long)v0);
  v3 = ((1) ? (unsigned long)((unsigned long)(unsigned long)((unsigned long)(unsigned long)a0[((unsigned char)h0() & 15)] >> 14) * (unsigned long)(unsigned long)v1) : (0));
  v4 = (unsigned short)((unsigned)(unsigned short)((unsigned)(unsigned short)(1) ^ (unsigned)(unsigned short)((unsigned char)((unsigned)(unsigned char)v1 << 7))) & (unsigned)(unsigned short)((unsigned)(unsigned short)((unsigned)(unsigned short)v2 + (unsigned)(unsigned short)v2) / ((unsigned)(((unsigned char)h0()) ? 0U : (unsigned short)v4) | 1)));
  switch (0) {
  default: ;
    ck = (0) ^ (unsigned short)((unsigned)(unsigned short)v4 ^ (unsigned)12500U);
  }
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v1 ^ (unsigned)(0);
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v3 ^ (unsigned)(v3 >> 8);
  return (unsigned short)((unsigned short)(0) ^ (unsigned short)ck);
}
