#include <stdio.h>

__attribute__((noinline)) unsigned long scratch_x19(unsigned long a) {
  unsigned long r;
  __asm__("mov x19, #5\n\tadd %0, %1, x19" : "=r"(r) : "r"(a) : "x19");
  return r;
}

__attribute__((noinline)) double scratch_d8(double a) {
  double r;
  __asm__("fmov d8, #2.0\n\tfadd %d0, %d1, d8" : "=w"(r) : "w"(a) : "d8");
  return r;
}

int main(int argc, char **argv) {
  unsigned long keep = argc + 1000;
  double dkeep = argc + 0.5;
  unsigned long r = scratch_x19(7);
  double d = scratch_d8(1.0);
  printf("%lu %lu %.1f %.1f\n", r, keep, d, dkeep);
  return 0;
}
