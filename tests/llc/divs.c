#include <stdio.h>
#include <stdint.h>
int main(void){
  volatile int32_t a=-1000, b=7; volatile uint32_t c=4000000000u, d=13; volatile int64_t e=-9000000000LL, f=11;
  printf("%ld %ld %lu %lu %lld %lld\n",(long)(a/b),(long)(a%b),(unsigned long)(c/d),(unsigned long)(c%d),(long long)(e/f),(long long)(e%f));
  return 0;
}
