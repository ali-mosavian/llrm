#include <stdio.h>

__attribute__((noinline)) unsigned long divmod(unsigned long a, unsigned long b, unsigned long *r) {
  unsigned long q, t;
  __asm__("udiv %0, %2, %3\n\tmsub %1, %0, %3, %2" : "=&r"(q), "=r"(t) : "r"(a), "r"(b));
  *r = t;
  return q;
}

__attribute__((noinline)) unsigned swap32(unsigned a) {
  unsigned r;
  __asm__("rev %w0, %w1" : "=r"(r) : "r"(a));
  return r;
}

__attribute__((noinline)) double root(double x) {
  double r;
  __asm__("fsqrt %d0, %d1" : "=w"(r) : "w"(x));
  return r;
}

__attribute__((noinline)) void store(unsigned *p, unsigned v) {
  __asm__ volatile("str %w1, %0" : "=m"(*p) : "r"(v));
}

__attribute__((noinline)) unsigned bump(unsigned a) {
  register unsigned r __asm__("w0") = a;
  __asm__ volatile("add w0, w0, #1" : "+r"(r));
  return r;
}

int main(void) {
  unsigned long r;
  unsigned long q = divmod(100, 7, &r);
  unsigned s = 0;
  store(&s, 77);
  printf("%lu %lu %x %.2f %u %u\n", q, r, swap32(0x11223344u), root(2.25), s, bump(41));
  return 0;
}
