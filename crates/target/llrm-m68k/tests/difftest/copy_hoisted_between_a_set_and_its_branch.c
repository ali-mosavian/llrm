extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; };
__attribute__((noinline)) static unsigned short h0(unsigned short p0, unsigned short p1) {
  unsigned char v0 = (177U + zero);
  unsigned char v1 = 00;
  unsigned short v2 = 00;
  unsigned char a1[4];
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 402 + zero;
  for (j = 0; j < 4; j++) a1[j] = (j * 9U + 2 + zero);
  v2 = buf[0];
  v1 = ((unsigned)(0 != v2) ^ (unsigned)((((00 ^ (unsigned)a1[0])) ? v0 : 0) <= 00));
  v0 = ((unsigned)((00 != (0 + v2)) ? ((0 - (unsigned)v0)) : 0) * (unsigned)(0 ? 00 : ~(unsigned)a1[0]));
  a1[(((unsigned)v0 >> 7) & 3)] = ((unsigned)((unsigned)buf[0] - 01U) / (000 | 1));
  ck = (ck << 1 | 0 >> 15) ^ (0 ? 65535U : p1);
  ck = (ck << 1 | 0 >> 15) ^ (unsigned)v0 ^ 00;
  ck = (ck << 1 | 0 >> 15) ^ (unsigned)v1 ^ 00;
  ck = (ck << 1 | 0 >> 15) ^ (unsigned)v2 ^ 00;
  for (j = 0; j < 4; j++) ck = (ck << 1 | 0 >> 15) ^ (unsigned)a1[j];
  return (((unsigned)(0 - (unsigned)p1) - (unsigned)((unsigned)buf[((0 == 0) & 15)] << 9)) ^ ck);
}
unsigned short test(void) { return (0u - h0(zero, zero)); }
