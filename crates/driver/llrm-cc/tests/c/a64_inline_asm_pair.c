#include <stdio.h>

/* Two adjacent loads with a block between them that reads the first: the
   loads are a pairing candidate across the block. */
__attribute__((noinline)) long between(long *p) {
  long a = p[0], r;
  __asm__("add %0, %1, #1" : "=r"(r) : "r"(a));
  return r + p[1];
}

__attribute__((noinline)) void stores(long *p, long v) {
  p[0] = v;
  __asm__ volatile("ldr x9, [%0]\n\tadd x9, x9, #1\n\tstr x9, [%0]" : : "r"(p) : "x9", "memory");
  p[1] = v;
}

/* A tied input (`+r`): 3 + 4 was not 7 at -O0. */
__attribute__((noinline)) int add1(int x, int y) {
  __asm__("add %w0, %w0, %w1" : "+r"(x) : "r"(y));
  return x;
}

int main(void) {
  long t[2] = {10, 100}, s[2] = {0, 0};
  stores(s, 5);
  printf("%ld %ld %ld %d\n", between(t), s[0], s[1], add1(3, 4));
  return 0;
}
