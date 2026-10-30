#include <stdio.h>
#include <stdint.h>
static unsigned popcount(uint64_t x){unsigned n=0;while(x){x&=x-1;n++;}return n;}
static uint32_t rotl(uint32_t x,int k){return (x<<k)|(x>>(32-k));}
int main(void){
  uint64_t v=0xF0F0123456789ABCULL; uint32_t w=0x80000001u;
  printf("%u %lu %u %llu %d\n",popcount(v),(unsigned long)rotl(w,3),(unsigned)(v>>60),(unsigned long long)(v^(v>>13)),(int)(int8_t)(v&0xff));
  return 0;
}
