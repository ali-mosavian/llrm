// flags: -O2 | -O3
// bar: none (a correctness test: a word times a small weight, multiplied narrow in a vector loop)
#include <stdio.h>

short back[2000];

long weighted(void) {
  long total = 0;
  for (long i = 0; i < 2000; i++)
    total += (long)((int)back[i] * ((i & 15) + 1));
  return total;
}

int main(void) {
  for (int i = 0; i < 2000; i++)
    back[i] = (short)(i * 37 - 30000);
  printf("%ld\n", weighted());
  return 0;
}
