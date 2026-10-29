extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static void bump(struct S *p, unsigned char x) { p->a = (unsigned char)(p->a + x); p->b = (unsigned short)(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }
unsigned short test(void) {
  unsigned char v0 = (unsigned char)(0);
  unsigned long v1 = (unsigned long)(0);
  unsigned short ck = 320 + zero;
  struct S s;
  s.a = 205 + zero; s.b = 44528 + zero; s.c = 171 + zero;
  v1 = 2147483647UL;
  v0 = (unsigned char)((unsigned)(unsigned char)((unsigned)127U ^ (unsigned)(unsigned char)(1)) + (unsigned)(unsigned char)((0) || ((unsigned char)s.a <= 1U)));
  if ((short)(unsigned short)((unsigned)(unsigned short)v0 / ((unsigned)(unsigned short)v0 | 1)) > (short)(unsigned short)((unsigned)(unsigned short)s.a * (unsigned)(unsigned short)v1)) {
    switch (0) {
    case 0: ;
      ck = (0) ^ (unsigned short)((unsigned)(unsigned short)v0 - (unsigned)(unsigned short)v1);
    }
  }
  v1 = (unsigned long)(0);
  switch (0) {
  case 0: ;
    v1 = (unsigned long)(v1 ^ (unsigned long)(unsigned long)((unsigned long)(unsigned long)(0) ^ (unsigned long)(unsigned long)v0));
  }
  return (unsigned short)((unsigned short)((unsigned)(unsigned short)((unsigned)(unsigned short)v1 << 4) & (unsigned)(unsigned short)(1)) ^ (unsigned short)ck);
}
