#include <stdio.h>
#include <stdint.h>
int main(void){
  volatile uint32_t a=3000000000u,b=7; volatile int32_t c=-5; volatile uint64_t d=18000000000000000000ULL;
  uint64_t m=(uint64_t)a*b;
  printf("%lu %ld %ld %llu %llu %d\n",(unsigned long)(a/b),(long)(c/2),(long)(c>>1),(unsigned long long)(d/1000),(unsigned long long)m,(int)(a>(uint32_t)c));
  return (int)(d%251);
}
