int printf(const char *, ...);
struct flags { unsigned a : 3, b : 5, c : 1; int d : 7; };
static unsigned pack(unsigned a, unsigned b, unsigned c) { struct flags f = {a, b, c, -3}; return f.a | f.b << 3 | f.c << 8 | (unsigned)f.d << 16; }
static int popcount(unsigned long x) { int n = 0; while (x) { x &= x - 1; n++; } return n; }
static unsigned rotl(unsigned x, int k) { return x << k | x >> (32 - k); }
static unsigned long rev(unsigned long x) { unsigned long r = 0; for (int i = 0; i < 64; i++) { r = r << 1 | (x & 1); x >>= 1; } return r; }
static int clz(unsigned x) { return __builtin_clz(x | 1); }
static int ctz(unsigned long x) { return __builtin_ctzl(x | 1ul << 63); }
static unsigned swapb(unsigned x) { return __builtin_bswap32(x); }
int main(void) {
  printf("%u %d %u\n", pack(5, 17, 1), popcount(0xF0F0F0F0F0F0F0F0ul), rotl(0x12345678u, 8));
  printf("%lx %d %d %x\n", rev(0x123456789abcdef0ul), clz(0x00ff0000u), ctz(0x4000ul), swapb(0x11223344u));
  unsigned x = 0xdeadbeef;
  printf("%u %u %u\n", x / 10, x % 1000, x / 7);
  int y = -123456;
  printf("%d %d %d\n", y / 10, y % 1000, y / 7);
  long z = -9876543210123L;
  printf("%ld %ld %ld\n", z / 1000, z % 1000, z / 13);
  return 0;
}
