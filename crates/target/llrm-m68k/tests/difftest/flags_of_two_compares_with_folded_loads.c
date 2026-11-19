extern const unsigned char buf[16];
extern const volatile unsigned char zero;
struct S { unsigned char a; unsigned short b; unsigned char c; };
static void bump(struct S *p, unsigned char x) { p->a = (unsigned char)(p->a + x); p->b = (unsigned short)(p->b ^ ((unsigned)x << 3)); if (x & 1) p->c = (unsigned char)(p->c + p->a); }
unsigned short test(void) {
  unsigned long v0 = (unsigned long)(457842489UL + zero);
  unsigned char v1 = (unsigned char)(1U + zero);
  unsigned char v2 = (unsigned char)(0);
  unsigned char j, i0, i1, i2, i3;
  unsigned short ck = 477 + zero;
  struct S s;
  s.a = 215 + zero; s.b = 31052 + zero; s.c = 190 + zero;
  v2 = (unsigned char)(0);
  if (0) {
    if (0) {
    }
  }
  bump(&s, (unsigned char)((unsigned char)((((unsigned char)v1) ? (unsigned char)v0 : (unsigned char)v2)) <= (unsigned char)(0)));
  for (i0 = 3; i0; i0--) {
    switch (0) {
    }
  }
  i0 = 4;
  while (i0) {
    i0--;
    i1 = 8;
    while (1) {
      bump(&s, (unsigned char)~(unsigned)(0));
      if (1) break;
    }
  }
  ck = (0) ^ s.a ^ s.b ^ s.c;
  return (unsigned short)((unsigned short)(0) ^ (unsigned short)ck);
}
