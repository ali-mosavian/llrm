/* the routine the m68k backend calls for a double-width multiply; shift and add, so it needs no multiply of its own */
unsigned long long __muldi3(unsigned long long a, unsigned long long b){
  unsigned long long r = 0;
  while (b) { if (b & 1) r += a; a <<= 1; b >>= 1; }
  return r;
}
int sf(int, int), sfs(int, int);
#ifdef __mc68020__
int sfs0(int, int);
#endif
unsigned uf(unsigned, unsigned), ufs(unsigned, unsigned);
static long sys3(long n,long a,long b,long c){register long d0 __asm__("d0")=n,d1 __asm__("d1")=a,d2 __asm__("d2")=b,d3 __asm__("d3")=c;__asm__ volatile("trap #0":"+d"(d0):"d"(d1),"d"(d2),"d"(d3):"memory");return d0;}
static const int v[] = { 0, 1, -1, 2, -2, 3, 32768, -32768, 49152, 65536, -65536, 0x12345678, -0x7654321, 0x7fffffff, -0x7fffffff - 1, 0x40000000, -0x40000000 };
#define N (int)(sizeof v / sizeof v[0])
int main(void){
  for (int i = 0; i < N; i++) for (int j = 0; j < N; j++) {
    int a = v[i], b = v[j];
    long long p = (long long)a * (long long)b;      /* the signed product, 64 bits */
    long long s = p >> 15;
    unsigned long long q = (unsigned long long)(unsigned)a * (unsigned long long)(unsigned)b;
    long long clamp = s > 0x7fffffffLL ? 0x7fffffffLL : s < -0x80000000LL ? -0x80000000LL : s;
    long long clamp0 = p > 0x7fffffffLL ? 0x7fffffffLL : p < -0x80000000LL ? -0x80000000LL : p;
    unsigned long long u = q >> 16;
    if (sf(a, b) != (int)s) return 1;
    if (sfs(a, b) != (int)clamp) return 2;
    if (uf((unsigned)a, (unsigned)b) != (unsigned)u) return 3;
    if (ufs((unsigned)a, (unsigned)b) != (u > 0xffffffffULL ? 0xffffffffU : (unsigned)u)) return 4;
#ifdef __mc68020__ /* the scale-0 saturating form multiplies with overflow, which is `muls.l` */
    if (sfs0(a, b) != (int)clamp0) return 5;
#endif
  }
  return 0;
}
void _start(void){ int r = main(); sys3(1, r, 0, 0); for(;;); }
