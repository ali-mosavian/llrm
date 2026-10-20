#include <stdio.h>
int main(void){
  volatile int a=-1000, b=7; volatile unsigned c=4000000000u, d=13; volatile long e=-9000000000L, f=11;
  printf("%d %d %u %u %ld %ld\n",a/b,a%b,c/d,c%d,e/f,e%f);
  return 0;
}
