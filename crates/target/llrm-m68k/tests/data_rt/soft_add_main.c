typedef unsigned long long u64;
u64 __adddf3(u64, u64), __subdf3(u64, u64);
static long sys3(long n,long a,long b,long c){register long d0 __asm__("d0")=n,d1 __asm__("d1")=a,d2 __asm__("d2")=b,d3 __asm__("d3")=c;__asm__ volatile("trap #0":"+d"(d0):"d"(d1),"d"(d2),"d"(d3):"memory");return d0;}
#define NX(s) ((s) ^= (s) << 13, (s) ^= (s) >> 7, (s) ^= (s) << 17, (s))
int main(void){
  unsigned h = 0; u64 seed = 88172645463325252ULL; int i;
  for (i = 0; i < 400; i++) {
    u64 a = NX(seed), b = NX(seed), r;
    if (i & 1) { a &= 0x800FFFFFFFFFFFFFULL; a |= (u64)(1000 + (NX(seed) & 31)) << 52; }
    if (i & 2) { b &= 0x800FFFFFFFFFFFFFULL; b |= (u64)(1000 + (NX(seed) & 31)) << 52; }
    r = (i & 4) ? __subdf3(a, b) : __adddf3(a, b);
    h = ((h << 5) ^ (h >> 27)) + (unsigned)r + ((unsigned)(r >> 32) ^ 0x9E3779B9u);
  }
  return ((h ^ (h >> 8) ^ (h >> 16) ^ (h >> 24)) & 255) == 96 ? 0 : 1;
}
void _start(void){ int r = main(); sys3(1, r, 0, 0); for(;;); }
