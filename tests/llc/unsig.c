#include <stdio.h>
int main(void){
  volatile unsigned a=3000000000u,b=7; volatile int c=-5; volatile unsigned long long d=18000000000000000000ULL;
  unsigned long long m=(unsigned long long)a*b;
  printf("%u %d %d %llu %llu %d\n",a/b,c/2,c>>1,d/1000,m,(int)(a>c));
  return (int)(d%251);
}
