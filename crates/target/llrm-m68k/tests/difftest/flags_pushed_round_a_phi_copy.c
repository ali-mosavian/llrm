extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static void bump(struct S *p, unsigned char x) { p->a = (unsigned char)(p->a + x); p->b = (unsigned short)(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }
unsigned short test(void) {
  unsigned char v0 = (unsigned char)(31U + zero);
  unsigned long v1 = (unsigned long)(1);
  unsigned long v2 = (unsigned long)(64UL + zero);
  unsigned long v3 = (unsigned long)(4294967295UL + zero);
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 926 + zero;
  struct S s;
  s.a = 161 + zero; s.b = 38068 + zero; s.c = 157 + zero;
  v0 = (unsigned char)((unsigned)(0) - (unsigned)(unsigned char)((unsigned)(unsigned char)((unsigned short)v0 > (unsigned short)v1) - (unsigned)(unsigned char)((unsigned)(unsigned char)v3 & (unsigned)(unsigned char)s.a)));
  v3 = (unsigned long)(v3 | (unsigned long)(unsigned long)(!((unsigned char)v2)));
  if (0) {
    if (0) {
    }
  }
  ck = (0) ^ (unsigned short)((unsigned)(unsigned short)buf[1] + (unsigned)(unsigned short)v0);
  v2 = (unsigned long)(0);
  for (i0 = 7; i0; i0--) {
    switch (0) {
    }
    if (0) {
    }
    ck = (0) ^ (unsigned short)((unsigned)(unsigned short)v3 - (unsigned)(unsigned short)v2);
  }
  while (0) {
    switch (0) {
    }
  }
  ck = (ck << 1 | ck >> 15) ^ (unsigned)v0 ^ (unsigned)(0);
  ck = (ck << 1 | ck >> 15) ^ s.a ^ s.b ^ s.c;
  return (unsigned short)((unsigned short)(0) ^ (unsigned short)ck);
}
