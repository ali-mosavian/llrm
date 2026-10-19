#include <stdio.h>
#include <stdint.h>
int main(void){
  uint64_t x=0x123456789abcdefULL;
  for(int i=0;i<10;i++) x=x*6364136223846793005ULL+1442695040888963407ULL;
  printf("%llu %lld %llu\n",(unsigned long long)x,(long long)(x>>7)-(long long)(x<<3),(unsigned long long)(x/1000003));
  return (int)(x&0x7f);
}
