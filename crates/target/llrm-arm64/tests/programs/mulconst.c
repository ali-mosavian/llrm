int printf(const char *, ...);
volatile unsigned long v = 0x123456789abcdeful;
volatile unsigned w = 0x89abcdefu;
int main(void) {
  unsigned long a = v;
  unsigned b = w;
  printf("%lx %lx %lx %lx\n", a * 0xFFFFFFFFFFFFFFFFul, a * 0x8000000000000001ul, a * 7, a * 9);
  printf("%x %x %x %x\n", b * 0xFFFFFFFFu, b * 0x80000001u, b * 7, b * 9);
  return 0;
}
